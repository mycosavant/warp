//! Owner-only Windows ACLs for local-control discovery artifacts and the
//! credential broker pipe.
//!
//! Upstream enforces the discovery registry's confidentiality with Unix mode
//! bits — `0o700` on the directory, `0o600` on each record and on the broker
//! socket — and refuses to publish at all on platforms that cannot express the
//! same guarantee. This module supplies that guarantee for Windows so the
//! refusal can be lifted, using a protected DACL that grants the calling user
//! and nobody else.
//!
//! "Protected" (`P` in SDDL) is the load-bearing part: it blocks inherited
//! ACEs from the parent directory, which is what makes the result equivalent
//! to a fresh `0o600` rather than "whatever the profile happens to allow".
//! Without it a permissive ACL inherited from `%LOCALAPPDATA%` would silently
//! widen access to artifacts we intend to keep owner-only.
//!
//! The same descriptor is reused for the named pipe, so the broker's transport
//! is protected by an ACL before any peer is authenticated — mirroring how the
//! Unix broker chmods its socket before accepting connections.

use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, SetFileSecurityW,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

use crate::{ControlError, ErrorCode};

fn security_error(operation: impl std::fmt::Display, error: impl std::fmt::Display) -> ControlError {
    ControlError::with_details(
        ErrorCode::Internal,
        format!("failed to {operation}"),
        error.to_string(),
    )
}

/// A security descriptor owned by this process, freed on drop.
///
/// `ConvertStringSecurityDescriptorToSecurityDescriptorW` allocates with
/// `LocalAlloc`, so the descriptor must outlive every use — notably the
/// `SECURITY_ATTRIBUTES` handed to pipe creation, which borrows rather than
/// copies it.
pub struct OwnerOnlySecurityDescriptor {
    descriptor: PSECURITY_DESCRIPTOR,
}

impl OwnerOnlySecurityDescriptor {
    /// Builds a protected DACL granting full access to the calling user alone.
    pub fn new() -> Result<Self, ControlError> {
        let sddl = format!("D:P(A;;GA;;;{})", current_user_sid_string()?);
        let wide: Vec<u16> = std::ffi::OsStr::new(&sddl)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(wide.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|err| security_error("build an owner-only security descriptor", err))?;
        Ok(Self { descriptor })
    }

    /// Security attributes borrowing this descriptor, for object creation.
    ///
    /// The returned value must not outlive `self`.
    pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor.0,
            bInheritHandle: false.into(),
        }
    }

    /// Replaces the DACL on an existing file or directory.
    pub fn apply_to(&self, path: &Path) -> Result<(), ControlError> {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            SetFileSecurityW(
                PCWSTR(wide.as_ptr()),
                DACL_SECURITY_INFORMATION,
                self.descriptor,
            )
        }
        .ok()
        .map_err(|err| {
            security_error(
                format!("apply an owner-only ACL to {}", path.display()),
                err,
            )
        })
    }
}

impl Drop for OwnerOnlySecurityDescriptor {
    fn drop(&mut self) {
        if !self.descriptor.is_invalid() {
            unsafe {
                let _ = LocalFree(Some(std::mem::transmute::<
                    *mut std::ffi::c_void,
                    windows::Win32::Foundation::HLOCAL,
                >(self.descriptor.0)));
            }
        }
    }
}

/// Applies an owner-only protected DACL to an existing path.
pub fn apply_owner_only_acl(path: &Path) -> Result<(), ControlError> {
    OwnerOnlySecurityDescriptor::new()?.apply_to(path)
}

/// The calling process's user SID in SDDL string form.
pub fn current_user_sid_string() -> Result<String, ControlError> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|err| security_error("open the current process token", err))?;
    let token = OwnedHandle(token);

    let user = token_user(token.0)?;
    let mut sid_string = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.sid(), &mut sid_string) }
        .map_err(|err| security_error("convert the current user SID to a string", err))?;
    let result = unsafe { sid_string.to_string() }
        .map_err(|err| security_error("decode the current user SID string", err));
    unsafe {
        let _ = LocalFree(Some(std::mem::transmute::<
            *mut u16,
            windows::Win32::Foundation::HLOCAL,
        >(sid_string.0)));
    }
    result
}

/// A `TOKEN_USER` and the buffer backing its interior SID pointer.
pub struct TokenUserBuffer(Vec<u8>);

impl TokenUserBuffer {
    /// The SID borrowed from the owned buffer.
    pub fn sid(&self) -> windows::Win32::Security::PSID {
        let user = self.0.as_ptr() as *const TOKEN_USER;
        unsafe { (*user).User.Sid }
    }
}

/// Reads `TokenUser` into an owned buffer sized by a probing call.
pub fn token_user(token: HANDLE) -> Result<TokenUserBuffer, ControlError> {
    use windows::Win32::Security::GetTokenInformation;

    let mut needed = 0u32;
    // Deliberately ignore the failure: the probing call is expected to fail
    // with ERROR_INSUFFICIENT_BUFFER, and its purpose is `needed`.
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut needed) };
    if needed == 0 {
        return Err(ControlError::new(
            ErrorCode::Internal,
            "failed to size the token user information buffer",
        ));
    }
    let mut buffer = vec![0u8; needed as usize];
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
            needed,
            &mut needed,
        )
    }
    .map_err(|err| security_error("read token user information", err))?;
    Ok(TokenUserBuffer(buffer))
}

/// Closes a token handle on drop.
pub struct OwnedHandle(pub HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
}
