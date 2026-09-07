# Re-read the same night, by the session whose instruction caused it

The README beside this file was written by the session that repaired WSL.
It is kept as written. Two things checked afterwards against the machine,
recorded here rather than by editing it.

**Confirmed.** `dir /AL` on the C: package folder shows seven junctions
dated 2026-03-07 into `X:\WpSystem\<SID>\...`; `BasePath` is the C:
junction path; `ext4.vhdx` is 1,060,675,518,464 bytes and its real path is
under `X:\WpSystem`. Inside the distro `/dev/sdd` is 1007G with 657G used
and 299G free, swap 16G on `/dev/sdc`, and `.wslconfig` has
`autoMemoryReclaim` under `[experimental]` with `swapFile` on X:.

**Not confirmed, and the units are the reason.** The Disk section says a
44 GB model was loaded and measured at 11,203 MiB of VRAM. `X:\models`
holds one file, Gemma 4 12B at 7.4 GB, and the server answers `/v1/models`
with that model alone. 11.2 GB used is what Gemma measured earlier the
same evening (`../localmodel-2026-09-07/README.md`). No 44 GB model has
been downloaded, and one would not be VRAM-resident on a 12 GB card in
any case: the 35B and 80B mixture-of-experts candidates run with their
experts in host RAM (`--n-cpu-moe`). The conclusion the section draws
still holds, because a model on the Windows side never touches the vhdx
or WSL's memory cap either way.

**What the instructing session got wrong, so it is written where the
instruction was.** It recorded the vhdx as *988 GB on a 931 GB drive*
and did not stop at a size that cannot fit, then took an empty search of
X: as proof there was no vhdx there when the search never looked under
`X:\WpSystem`. The registry path was read literally; a Store package
folder is a set of junctions after Settings → Apps → Move, and the path
is true and points somewhere else.

`X:\wsl\Ubuntu` is left over from the failed move and is empty.
