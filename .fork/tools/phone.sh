#!/bin/bash
# The Android emulator on the Windows side, driven from WSL. It stands in
# for the phone on the checklist at the top of .fork/HANDOFF-MOBILE.md: it
# is a Windows process, so it reaches the wide listener the way a phone on
# the LAN does (WSL itself cannot, measured). The AVD is `warp_phone`,
# an android-36 Google APIs x86_64 image under C:\Users\onemind\.android\avd,
# written by hand on 2026-09-06 because the Windows SDK has no avdmanager.
#
#   phone.sh start [cold]   boot it (cold: ignore the quick-boot snapshot)
#   phone.sh wait           block until Android reports boot complete
#   phone.sh stop           power it off (the quick-boot snapshot is saved)
#   phone.sh shot <file>    screenshot to <file>.png
#   phone.sh open <url>     open a URL in the default browser
#   phone.sh tap <x> <y>    tap; phone.sh type <text>; phone.sh key <keycode>
#   phone.sh tapon <text>   tap the first element whose label contains <text>
#   phone.sh find <text>    the same, printing its centre without tapping
#   phone.sh ui             dump the UI tree
#   phone.sh adb ...        anything else
set -u
SDK=/mnt/c/Users/onemind/AppData/Local/Android/Sdk
AVD=${AVD:-warp_phone}
adbraw() { (cd /mnt/c && "$SDK/platform-tools/adb.exe" -s emulator-5554 "$@"); }
adb() { adbraw "$@" | tr -d '\r'; }
case "${1:-}" in
  start)
    shift; SNAP=""; [ "${1:-}" = cold ] && SNAP="'-no-snapshot-load',"
    powershell.exe -NoProfile -Command "Start-Process -FilePath 'C:\Users\onemind\AppData\Local\Android\Sdk\emulator\emulator.exe' -ArgumentList '-avd','$AVD',$SNAP'-no-boot-anim','-no-metrics','-netdelay','none','-netspeed','full','-gpu','auto' -RedirectStandardOutput 'C:\dev\phone\emulator.out' -RedirectStandardError 'C:\dev\phone\emulator.err'" ;;
  wait)
    adb wait-for-device >/dev/null
    for i in $(seq 1 120); do [ "$(adb shell getprop sys.boot_completed 2>/dev/null)" = 1 ] && { echo "booted after ~$((i*3)) s"; exit 0; }; sleep 3; done
    echo "not booted after 360 s" >&2; exit 1 ;;
  stop) adb emu kill ;;
  shot) adbraw exec-out screencap -p > "$2.png" 2>/dev/null; ls -la "$2.png" | awk '{print $5, $9}' ;;
  open) adb shell am start -a android.intent.action.VIEW -d "'$2'" ;;
  tap) adb shell input tap "$2" "$3" ;;
  type) adb shell input text "'$2'" ;;
  key) adb shell input keyevent "$2" ;;
  ui) adb shell 'uiautomator dump /sdcard/ui.xml >/dev/null && cat /sdcard/ui.xml' ;;
  scroll) adb shell input swipe 540 1700 540 700 300 ;;
  find|tapon)  # the centre of the first node whose text or content-desc contains $2; tapon scrolls down up to 5 times looking for it
    for try in 1 2 3 4 5 6; do
    XML=$(adb shell 'uiautomator dump /sdcard/ui.xml >/dev/null && cat /sdcard/ui.xml')
    XY=$(printf '%s' "$XML" | python3 -c '
import sys,re,xml.etree.ElementTree as ET
needle=sys.argv[1].lower(); nodes=list(ET.fromstring(sys.stdin.read()).iter("node"))
# exact text, then text containing, then content-desc containing: a row beats its preview button
hit=None
for pick in (lambda n: n.get("text","").lower()==needle, lambda n: needle in n.get("text","").lower(), lambda n: needle in n.get("content-desc","").lower()):
    hit=next((n for n in nodes if pick(n)),None)
    if hit is not None: break  # an Element with no children is falsy, so never test it with or
if hit is not None:
    x1,y1,x2,y2=map(int,re.findall(r"\d+",hit.get("bounds")))
    print((x1+x2)//2,(y1+y2)//2, "|", (hit.get("text") or hit.get("content-desc"))[:60])
' "$2")
    [ -n "$XY" ] && break; [ "$1" = find ] && break; adb shell input swipe 540 1700 540 700 300; sleep 1
    done
    [ -z "$XY" ] && { echo "not on screen: $2" >&2; exit 1; }
    echo "$XY"; if [ "$1" = tapon ]; then adb shell input tap ${XY%% |*}; fi ;;
  adb) shift; adb "$@" ;;
  *) sed -n '2,15p' "$0"; exit 2 ;;
esac
