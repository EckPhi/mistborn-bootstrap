#!/usr/bin/env bash
set -Eeuo pipefail
trap 'ui_error "Setup failed on line $LINENO"' ERR
MISTBORN_DRY_RUN=0
MISTBORN_YES=0
MISTBORN_ONLY=""
MISTBORN_TASKS=""
MISTBORN_MODULES=( common docker zsh tailscale runtipi rclone security toolset )
export MISTBORN_TOOL_B64='IyEvdXNyL2Jpbi9lbnYgYmFzaApzZXQgLUVldW8gcGlwZWZhaWwKClJVTlRJUElfUEFUSD0iJHtSVU5USVBJX1BBVEg6LS9vcHQvcnVudGlwaX0iCk1JU1RCT1JOX1NUQVRFX0RJUj0iJHtNSVNUQk9STl9TVEFURV9ESVI6LS92YXIvbGliL21pc3Rib3JuLWJvb3RzdHJhcH0iCk1JU1RCT1JOX1JVTk5FUl9QQVRIPSIke01JU1RCT1JOX1JVTk5FUl9QQVRIOi0vdXNyL2xvY2FsL2Jpbi9taXN0Ym9ybi1ib290c3RyYXB9IgpNSVNUQk9STl9QTEVYX1VGV19QUk9GSUxFPSIke01JU1RCT1JOX1BMRVhfVUZXX1BST0ZJTEU6LS9ldGMvdWZ3L2FwcGxpY2F0aW9ucy5kL3BsZXhtZWRpYXNlcnZlcn0iCgpkaWUoKSB7IHByaW50ZiAnZXJyb3I6ICVzXG4nICIkKiIgPiYyOyBleGl0IDE7IH0KdGFza19ldmVudCgpIHsKICBbWyAtbiAiJHtNSVNUQk9STl9QUk9HUkVTU19GSUxFOi19IiBdXSB8fCByZXR1cm4gMAogIHByaW50ZiAnJXNcdCVzXHQlc1xuJyAiJDEiICIkMiIgIiQzIiA+PiIkTUlTVEJPUk5fUFJPR1JFU1NfRklMRSIgfHwgdHJ1ZQp9CnJ1bnRpcGlfY2xpKCkgewogIGlmIFtbIC14ICIkUlVOVElQSV9QQVRIL3J1bnRpcGktY2xpIiBdXTsgdGhlbiBwcmludGYgJyVzXG4nICIkUlVOVElQSV9QQVRIL3J1bnRpcGktY2xpIgogIGVsaWYgY29tbWFuZCAtdiBydW50aXBpLWNsaSA+L2Rldi9udWxsOyB0aGVuIGNvbW1hbmQgLXYgcnVudGlwaS1jbGkKICBlbHNlIGRpZSAicnVudGlwaS1jbGkgbm90IGZvdW5kIHVuZGVyICRSVU5USVBJX1BBVEggb3IgUEFUSCI7IGZpCn0KcnVuX3J1bnRpcGkoKSB7ICIkKHJ1bnRpcGlfY2xpKSIgIiRAIjsgfQphcHBfcmVmcygpIHsKICBsb2NhbCBzdG9yZSBhcHAKICBmb3Igc3RvcmUgaW4gIiRSVU5USVBJX1BBVEgiL2FwcHMvKjsgZG8KICAgIFtbIC1kICIkc3RvcmUiIF1dIHx8IGNvbnRpbnVlCiAgICBmb3IgYXBwIGluICIkc3RvcmUiLyo7IGRvIFtbIC1kICIkYXBwIiBdXSAmJiBwcmludGYgJyVzOiVzXG4nICIkKGJhc2VuYW1lICIkYXBwIikiICIkKGJhc2VuYW1lICIkc3RvcmUiKSI7IGRvbmUKICBkb25lCn0Kc25hcHNob3RfYXBwcygpIHsgbG9jYWwgcmVmOyB3aGlsZSBJRlM9IHJlYWQgLXIgcmVmOyBkbyBydW5fcnVudGlwaSBhcHAgYmFja3VwICIkcmVmIjsgZG9uZTsgfQoKcmVwb3J0X2NvbW1hbmRfdmVyc2lvbigpIHsKICBsb2NhbCBjb21tYW5kPSIkMSIgdmVyc2lvbgogIGlmICEgY29tbWFuZCAtdiAiJGNvbW1hbmQiID4vZGV2L251bGwgMj4mMTsgdGhlbgogICAgcHJpbnRmICcgIFdBUk4gICVzIG5vdCBpbnN0YWxsZWRcbicgIiRjb21tYW5kIgogICAgcmV0dXJuIDAKICBmaQogIHZlcnNpb249IiQoJGNvbW1hbmQgLS12ZXJzaW9uIDI+JjEgfCBoZWFkIC1uMSB8fCB0cnVlKSIKICBbWyAtbiAiJHZlcnNpb24iIF1dIHx8IHZlcnNpb249Imluc3RhbGxlZCAodmVyc2lvbiB1bmF2YWlsYWJsZSkiCiAgcHJpbnRmICcgIFBBU1MgICVzOiAlc1xuJyAiJGNvbW1hbmQiICIkdmVyc2lvbiIKfQoKYm9vdHN0cmFwX3N0YXRlX3N0YXR1cygpIHsKICBsb2NhbCBzdGF0ZV9maWxlPSIkTUlTVEJPUk5fU1RBVEVfRElSL3NlcnZlci5qc29uIgogIHByaW50ZiAnXG5Cb290c3RyYXAgc3RhdGVcbicKICBpZiBbWyAhIC1yICIkc3RhdGVfZmlsZSIgXV07IHRoZW4KICAgIHByaW50ZiAnICBXQVJOICBubyByZWFkYWJsZSBzZXJ2ZXIgc3RhdGUgYXQgJXNcbicgIiRzdGF0ZV9maWxlIgogICAgcmV0dXJuIDAKICBmaQogIGlmICEgY29tbWFuZCAtdiBweXRob24zID4vZGV2L251bGwgMj4mMTsgdGhlbgogICAgcHJpbnRmICcgIFdBUk4gIHB5dGhvbjMgdW5hdmFpbGFibGU7IGNhbm5vdCBpbnNwZWN0ICVzXG4nICIkc3RhdGVfZmlsZSIKICAgIHJldHVybiAwCiAgZmkKICBweXRob24zIC0gIiRzdGF0ZV9maWxlIiA8PCdQWScKaW1wb3J0IGpzb24KaW1wb3J0IHN5cwpmcm9tIGRhdGV0aW1lIGltcG9ydCBkYXRldGltZSwgdGltZXpvbmUKCnBhdGggPSBzeXMuYXJndlsxXQp0cnk6CiAgICB3aXRoIG9wZW4ocGF0aCwgZW5jb2Rpbmc9InV0Zi04IikgYXMgaGFuZGxlOgogICAgICAgIHN0YXRlID0ganNvbi5sb2FkKGhhbmRsZSkKZXhjZXB0IChPU0Vycm9yLCBWYWx1ZUVycm9yKSBhcyBlcnJvcjoKICAgIHByaW50KGYiICBGQUlMICBzdGF0ZSB1bnJlYWRhYmxlOiB7ZXJyb3J9IikKICAgIHJhaXNlIFN5c3RlbUV4aXQoMSkKCnZlcnNpb24gPSBzdGF0ZS5nZXQoInZlcnNpb24iLCAidW5rbm93biIpCnVwZGF0ZWQgPSBzdGF0ZS5nZXQoInVwZGF0ZWRfYXQiKQpzdGFtcCA9ICJ1bmtub3duIgppZiBpc2luc3RhbmNlKHVwZGF0ZWQsIChpbnQsIGZsb2F0KSk6CiAgICBzdGFtcCA9IGRhdGV0aW1lLmZyb210aW1lc3RhbXAodXBkYXRlZCwgdHo9dGltZXpvbmUudXRjKS5pc29mb3JtYXQoKQpwcmludChmIiAgUEFTUyAgc2NoZW1hIHZ7dmVyc2lvbn07IHVwZGF0ZWQge3N0YW1wfSIpCmZhaWxlZCA9IEZhbHNlCmZvciBuYW1lLCBtb2R1bGUgaW4gc3RhdGUuZ2V0KCJtb2R1bGVzIiwge30pLml0ZW1zKCk6CiAgICBzdGF0dXMgPSBtb2R1bGUuZ2V0KCJzdGF0dXMiLCAidW5rbm93biIpCiAgICBtYXJrZXIgPSAiUEFTUyIgaWYgc3RhdHVzID09ICJjb21wbGV0ZWQiIGVsc2UgIldBUk4iCiAgICBpZiBzdGF0dXMgPT0gImZhaWxlZCI6CiAgICAgICAgbWFya2VyID0gIkZBSUwiCiAgICAgICAgZmFpbGVkID0gVHJ1ZQogICAgcHJpbnQoZiIgIHttYXJrZXI6PDR9ICB7bmFtZX06IHtzdGF0dXN9IikKcmFpc2UgU3lzdGVtRXhpdCgxIGlmIGZhaWxlZCBlbHNlIDApClBZCn0KCnN0YXRlX3Rhc2tfc3RhdHVzKCkgewogIGxvY2FsIG1vZHVsZT0iJDEiIHRhc2s9IiQyIiBzdGF0ZV9maWxlPSIkTUlTVEJPUk5fU1RBVEVfRElSL3NlcnZlci5qc29uIgogIGlmIFtbICEgLXIgIiRzdGF0ZV9maWxlIiBdXSB8fCAhIGNvbW1hbmQgLXYgcHl0aG9uMyA+L2Rldi9udWxsIDI+JjE7IHRoZW4KICAgIHByaW50ZiAndW5rbm93blxuJwogICAgcmV0dXJuIDAKICBmaQogIHB5dGhvbjMgLSAiJHN0YXRlX2ZpbGUiICIkbW9kdWxlIiAiJHRhc2siIDw8J1BZJyAyPi9kZXYvbnVsbCB8fCBwcmludGYgJ3Vua25vd25cbicKaW1wb3J0IGpzb24KaW1wb3J0IHN5cwp0cnk6CiAgICB3aXRoIG9wZW4oc3lzLmFyZ3ZbMV0sIGVuY29kaW5nPSJ1dGYtOCIpIGFzIGhhbmRsZToKICAgICAgICBzdGF0ZSA9IGpzb24ubG9hZChoYW5kbGUpCiAgICB0YXNrID0gc3RhdGUuZ2V0KCJtb2R1bGVzIiwge30pLmdldChzeXMuYXJndlsyXSwge30pLmdldCgidGFza3MiLCB7fSkuZ2V0KHN5cy5hcmd2WzNdLCB7fSkKICAgIHByaW50KHRhc2sgaWYgaXNpbnN0YW5jZSh0YXNrLCBzdHIpIGVsc2UgdGFzay5nZXQoInN0YXR1cyIsICJ1bmtub3duIikpCmV4Y2VwdCAoT1NFcnJvciwgVmFsdWVFcnJvcik6CiAgICBwcmludCgidW5rbm93biIpClBZCn0KCmNvbmZpZ3VyYXRpb25fc3RhdHVzKCkgewogIGxvY2FsIGZhaWx1cmVzPTAgc3NoX3BvbGljeSB1ZndfcG9saWN5IHRhaWxzY2FsZV9qc29uIHRhaWxzY2FsZV9wcmVmcwogIGxvY2FsIHNzaF9leHBlY3RlZCBmaXJld2FsbF9leHBlY3RlZCBwbGV4X2V4cGVjdGVkIGZhaWwyYmFuX2V4cGVjdGVkIGZvcndhcmRpbmdfZXhwZWN0ZWQgdGFpbHNjYWxlX29ubHlfZXhwZWN0ZWQKICBzc2hfZXhwZWN0ZWQ9IiQoc3RhdGVfdGFza19zdGF0dXMgc2VjdXJpdHkgc3NoKSIKICBmaXJld2FsbF9leHBlY3RlZD0iJChzdGF0ZV90YXNrX3N0YXR1cyBzZWN1cml0eSBmaXJld2FsbCkiCiAgcGxleF9leHBlY3RlZD0iJChzdGF0ZV90YXNrX3N0YXR1cyBzZWN1cml0eSBwbGV4LWZpcmV3YWxsKSIKICBmYWlsMmJhbl9leHBlY3RlZD0iJChzdGF0ZV90YXNrX3N0YXR1cyBzZWN1cml0eSBmYWlsMmJhbikiCiAgZm9yd2FyZGluZ19leHBlY3RlZD0iJChzdGF0ZV90YXNrX3N0YXR1cyB0YWlsc2NhbGUgZm9yd2FyZGluZykiCiAgdGFpbHNjYWxlX29ubHlfZXhwZWN0ZWQ9IiQoc3RhdGVfdGFza19zdGF0dXMgc2VjdXJpdHkgdGFpbHNjYWxlLW9ubHkpIgogIHByaW50ZiAnXG5FZmZlY3RpdmUgY29uZmlndXJhdGlvblxuJwoKICBpZiBjb21tYW5kIC12IHNzaGQgPi9kZXYvbnVsbCAyPiYxOyB0aGVuCiAgICBzc2hfcG9saWN5PSIkKHNzaGQgLVQgMj4vZGV2L251bGwgfHwgdHJ1ZSkiCiAgICBpZiBncmVwIC1xeCAncGFzc3dvcmRhdXRoZW50aWNhdGlvbiBubycgPDw8IiRzc2hfcG9saWN5IjsgdGhlbgogICAgICBwcmludGYgJyAgUEFTUyAgU1NIIHBhc3N3b3JkIGF1dGhlbnRpY2F0aW9uIGRpc2FibGVkXG4nCiAgICBlbGlmIFtbICIkc3NoX2V4cGVjdGVkIiA9PSBjb21wbGV0ZWQgXV07IHRoZW4KICAgICAgcHJpbnRmICcgIEZBSUwgIFNTSCBwYXNzd29yZCBhdXRoZW50aWNhdGlvbiBkcmlmdGVkIGZyb20gYXBwbGllZCBoYXJkZW5pbmdcbicKICAgICAgZmFpbHVyZXM9MQogICAgZWxzZQogICAgICBwcmludGYgJyAgV0FSTiAgU1NIIHBhc3N3b3JkIGF1dGhlbnRpY2F0aW9uIGlzIG5vdCBkaXNhYmxlZFxuJwogICAgZmkKICAgIGlmIGdyZXAgLXF4ICdwZXJtaXRyb290bG9naW4gbm8nIDw8PCIkc3NoX3BvbGljeSI7IHRoZW4KICAgICAgcHJpbnRmICcgIFBBU1MgIFNTSCByb290IGxvZ2luIGRpc2FibGVkXG4nCiAgICBlbGlmIFtbICIkc3NoX2V4cGVjdGVkIiA9PSBjb21wbGV0ZWQgXV07IHRoZW4KICAgICAgcHJpbnRmICcgIEZBSUwgIFNTSCByb290LWxvZ2luIHBvbGljeSBkcmlmdGVkIGZyb20gYXBwbGllZCBoYXJkZW5pbmdcbicKICAgICAgZmFpbHVyZXM9MQogICAgZWxzZQogICAgICBwcmludGYgJyAgV0FSTiAgU1NIIHJvb3QgbG9naW4gaXMgbm90IGZ1bGx5IGRpc2FibGVkXG4nCiAgICBmaQogICAgcHJpbnRmICcgIElORk8gIFNTSCAlc1xuJyAiJChncmVwIC1tMSAnXnBvcnQgJyA8PDwiJHNzaF9wb2xpY3kiIHx8IHByaW50ZiAncG9ydCB1bmtub3duJykiCiAgZWxzZQogICAgcHJpbnRmICcgIFdBUk4gIHNzaGQgdW5hdmFpbGFibGU7IFNTSCBwb2xpY3kgbm90IGNoZWNrZWRcbicKICBmaQoKICBpZiBjb21tYW5kIC12IHVmdyA+L2Rldi9udWxsIDI+JjE7IHRoZW4KICAgIHVmd19wb2xpY3k9IiQodWZ3IHN0YXR1cyB2ZXJib3NlIDI+L2Rldi9udWxsIHx8IHRydWUpIgogICAgaWYgZ3JlcCAtcSAnXlN0YXR1czogYWN0aXZlJyA8PDwiJHVmd19wb2xpY3kiOyB0aGVuCiAgICAgIHByaW50ZiAnICBQQVNTICBVRlcgYWN0aXZlXG4nCiAgICBlbGlmIFtbICIkZmlyZXdhbGxfZXhwZWN0ZWQiID09IGNvbXBsZXRlZCBdXTsgdGhlbgogICAgICBwcmludGYgJyAgRkFJTCAgVUZXIGluYWN0aXZlIGFmdGVyIGZpcmV3YWxsIHRhc2sgd2FzIGFwcGxpZWRcbicKICAgICAgZmFpbHVyZXM9MQogICAgZWxzZQogICAgICBwcmludGYgJyAgV0FSTiAgVUZXIGluYWN0aXZlXG4nCiAgICBmaQogICAgaWYgZ3JlcCAtRXEgJ15EZWZhdWx0OiBkZW55IFwoaW5jb21pbmdcKScgPDw8IiR1ZndfcG9saWN5IjsgdGhlbgogICAgICBwcmludGYgJyAgUEFTUyAgVUZXIGRlZmF1bHQgaW5jb21pbmcgcG9saWN5IGlzIGRlbnlcbicKICAgIGVsaWYgW1sgIiRmaXJld2FsbF9leHBlY3RlZCIgPT0gY29tcGxldGVkIF1dOyB0aGVuCiAgICAgIHByaW50ZiAnICBGQUlMICBVRlcgaW5jb21pbmcgcG9saWN5IGRyaWZ0ZWQgZnJvbSBhcHBsaWVkIGhhcmRlbmluZ1xuJwogICAgICBmYWlsdXJlcz0xCiAgICBlbHNlCiAgICAgIHByaW50ZiAnICBXQVJOICBVRlcgZGVmYXVsdCBpbmNvbWluZyBwb2xpY3kgaXMgbm90IGRlbnlcbicKICAgIGZpCiAgICBpZiBbWyAiJHBsZXhfZXhwZWN0ZWQiID09IGNvbXBsZXRlZCAmJiAhIC1mICIkTUlTVEJPUk5fUExFWF9VRldfUFJPRklMRSIgXV07IHRoZW4KICAgICAgcHJpbnRmICcgIEZBSUwgIGFwcGxpZWQgUGxleCBVRlcgcHJvZmlsZSBpcyBtaXNzaW5nXG4nCiAgICAgIGZhaWx1cmVzPTEKICAgIGVsaWYgW1sgLWYgIiRNSVNUQk9STl9QTEVYX1VGV19QUk9GSUxFIiBdXTsgdGhlbgogICAgICBpZiBncmVwIC1FcSAnXjMyNDAwL3RjcFtbOnNwYWNlOl1dK0FMTE9XJyA8PDwiJHVmd19wb2xpY3kiOyB0aGVuCiAgICAgICAgcHJpbnRmICcgIFBBU1MgIFBsZXggcmVtb3RlLWFjY2VzcyBydWxlIHByZXNlbnRcbicKICAgICAgZWxzZQogICAgICAgIHByaW50ZiAnICBGQUlMICBQbGV4IHByb2ZpbGUgaW5zdGFsbGVkIGJ1dCBUQ1AgMzI0MDAgYWxsb3cgcnVsZSBtaXNzaW5nXG4nCiAgICAgICAgZmFpbHVyZXM9MQogICAgICBmaQogICAgICBpZiBncmVwIC1FcSAncGxleG1lZGlhc2VydmVyLWFsbC4qdGFpbHNjYWxlMHx0YWlsc2NhbGUwLipwbGV4bWVkaWFzZXJ2ZXItYWxsJyA8PDwiJHVmd19wb2xpY3kiOyB0aGVuCiAgICAgICAgcHJpbnRmICcgIFBBU1MgIFBsZXggbG9jYWwgc2VydmljZXMgYWxsb3dlZCB0aHJvdWdoIHRhaWxzY2FsZTBcbicKICAgICAgZWxzZQogICAgICAgIHByaW50ZiAnICBJTkZPICBubyBQbGV4IGxvY2FsLXNlcnZpY2VzIHJ1bGUgb24gdGFpbHNjYWxlMFxuJwogICAgICBmaQogICAgZmkKICBlbHNlCiAgICBwcmludGYgJyAgV0FSTiAgVUZXIHVuYXZhaWxhYmxlOyBmaXJld2FsbCBwb2xpY3kgbm90IGNoZWNrZWRcbicKICBmaQoKICBpZiBjb21tYW5kIC12IGZhaWwyYmFuLWNsaWVudCA+L2Rldi9udWxsIDI+JjE7IHRoZW4KICAgIGlmIGZhaWwyYmFuLWNsaWVudCBzdGF0dXMgc3NoZCA+L2Rldi9udWxsIDI+JjE7IHRoZW4KICAgICAgcHJpbnRmICcgIFBBU1MgIGZhaWwyYmFuIHNzaGQgamFpbCBhY3RpdmVcbicKICAgIGVsaWYgW1sgIiRmYWlsMmJhbl9leHBlY3RlZCIgPT0gY29tcGxldGVkIF1dOyB0aGVuCiAgICAgIHByaW50ZiAnICBGQUlMICBmYWlsMmJhbiBzc2hkIGphaWwgaW5hY3RpdmUgYWZ0ZXIgdGFzayB3YXMgYXBwbGllZFxuJwogICAgICBmYWlsdXJlcz0xCiAgICBlbHNlCiAgICAgIHByaW50ZiAnICBXQVJOICBmYWlsMmJhbiBzc2hkIGphaWwgdW5hdmFpbGFibGVcbicKICAgIGZpCiAgZmkKCiAgaWYgY29tbWFuZCAtdiB0YWlsc2NhbGUgPi9kZXYvbnVsbCAyPiYxOyB0aGVuCiAgICB0YWlsc2NhbGVfanNvbj0iJCh0YWlsc2NhbGUgc3RhdHVzIC0tanNvbiAyPi9kZXYvbnVsbCB8fCB0cnVlKSIKICAgIGlmIGdyZXAgLUVxICciQmFja2VuZFN0YXRlIltbOnNwYWNlOl1dKjpbWzpzcGFjZTpdXSoiUnVubmluZyInIDw8PCIkdGFpbHNjYWxlX2pzb24iOyB0aGVuCiAgICAgIHByaW50ZiAnICBQQVNTICBUYWlsc2NhbGUgYmFja2VuZCBydW5uaW5nXG4nCiAgICBlbHNlCiAgICAgIHByaW50ZiAnICBGQUlMICBUYWlsc2NhbGUgYmFja2VuZCBub3QgcnVubmluZ1xuJwogICAgICBmYWlsdXJlcz0xCiAgICBmaQogICAgdGFpbHNjYWxlX3ByZWZzPSIkKHRhaWxzY2FsZSBkZWJ1ZyBwcmVmcyAyPi9kZXYvbnVsbCB8fCB0cnVlKSIKICAgIGlmIGdyZXAgLUVxICciUnVuU1NIIltbOnNwYWNlOl1dKjpbWzpzcGFjZTpdXSp0cnVlJyA8PDwiJHRhaWxzY2FsZV9wcmVmcyI7IHRoZW4KICAgICAgcHJpbnRmICcgIElORk8gIFRhaWxzY2FsZSBTU0ggZW5hYmxlZFxuJwogICAgZWxpZiBbWyAiJHRhaWxzY2FsZV9vbmx5X2V4cGVjdGVkIiA9PSBjb21wbGV0ZWQgXV07IHRoZW4KICAgICAgcHJpbnRmICcgIEZBSUwgIFRhaWxzY2FsZSBTU0ggZGlzYWJsZWQgYWZ0ZXIgVGFpbHNjYWxlLW9ubHkgdGFzayB3YXMgYXBwbGllZFxuJwogICAgICBmYWlsdXJlcz0xCiAgICBlbHNlCiAgICAgIHByaW50ZiAnICBJTkZPICBUYWlsc2NhbGUgU1NIIGRpc2FibGVkIG9yIHVuYXZhaWxhYmxlXG4nCiAgICBmaQogICAgaWYgZ3JlcCAtRXEgJyJBZHZlcnRpc2VSb3V0ZXMiW1s6c3BhY2U6XV0qOltbOnNwYWNlOl1dKlxbW15dXScgPDw8IiR0YWlsc2NhbGVfcHJlZnMiOyB0aGVuCiAgICAgIHByaW50ZiAnICBJTkZPICBUYWlsc2NhbGUgcm91dGVzIGFyZSBhZHZlcnRpc2VkXG4nCiAgICBlbGlmIFtbICIkZm9yd2FyZGluZ19leHBlY3RlZCIgPT0gY29tcGxldGVkIF1dOyB0aGVuCiAgICAgIHByaW50ZiAnICBGQUlMICBubyByb3V0ZXMgYWR2ZXJ0aXNlZCBhZnRlciBleGl0LW5vZGUgZm9yd2FyZGluZyB0YXNrIHdhcyBhcHBsaWVkXG4nCiAgICAgIGZhaWx1cmVzPTEKICAgIGVsc2UKICAgICAgcHJpbnRmICcgIElORk8gIG5vIFRhaWxzY2FsZSByb3V0ZXMgYWR2ZXJ0aXNlZFxuJwogICAgZmkKICBlbHNlCiAgICBwcmludGYgJyAgV0FSTiAgVGFpbHNjYWxlIHVuYXZhaWxhYmxlOyBjb25maWd1cmF0aW9uIG5vdCBjaGVja2VkXG4nCiAgZmkKICByZXR1cm4gIiRmYWlsdXJlcyIKfQoKZG9jdG9yKCkgewogIGxvY2FsIGZhaWx1cmVzPTAKICBwcmludGYgJ01pc3Rib3JuIGhvc3QgZGlhZ25vc3RpY3NcbicKICBmb3IgY29tbWFuZCBpbiBkb2NrZXIgdGFpbHNjYWxlIHJjbG9uZSB1ZncgZmFpbDJiYW4tY2xpZW50OyBkbwogICAgaWYgY29tbWFuZCAtdiAiJGNvbW1hbmQiID4vZGV2L251bGwgMj4mMTsgdGhlbiBwcmludGYgJyAgUEFTUyAgJXMgaW5zdGFsbGVkXG4nICIkY29tbWFuZCI7IGVsc2UgcHJpbnRmICcgIFdBUk4gICVzIG1pc3NpbmdcbicgIiRjb21tYW5kIjsgZmkKICBkb25lCiAgaWYgW1sgLWQgIiRSVU5USVBJX1BBVEgiIF1dOyB0aGVuIHByaW50ZiAnICBQQVNTICBSdW50aXBpIGRpcmVjdG9yeTogJXNcbicgIiRSVU5USVBJX1BBVEgiOyBlbHNlIHByaW50ZiAnICBGQUlMICBSdW50aXBpIGRpcmVjdG9yeSBtaXNzaW5nXG4nOyBmYWlsdXJlcz0xOyBmaQogIGlmIGRvY2tlciBpbmZvID4vZGV2L251bGwgMj4mMTsgdGhlbiBwcmludGYgJyAgUEFTUyAgRG9ja2VyIGRhZW1vbiByZWFjaGFibGVcbic7IGVsc2UgcHJpbnRmICcgIEZBSUwgIERvY2tlciBkYWVtb24gdW5yZWFjaGFibGVcbic7IGZhaWx1cmVzPTE7IGZpCiAgaWYgc3lzdGVtY3RsIGlzLWFjdGl2ZSAtLXF1aWV0IGZhaWwyYmFuOyB0aGVuIHByaW50ZiAnICBQQVNTICBmYWlsMmJhbiBhY3RpdmVcbic7IGVsc2UgcHJpbnRmICcgIFdBUk4gIGZhaWwyYmFuIGluYWN0aXZlXG4nOyBmaQogIGlmIHVmdyBzdGF0dXMgMj4vZGV2L251bGwgfCBncmVwIC1xICdTdGF0dXM6IGFjdGl2ZSc7IHRoZW4gcHJpbnRmICcgIFBBU1MgIFVGVyBhY3RpdmVcbic7IGVsc2UgcHJpbnRmICcgIFdBUk4gIFVGVyBpbmFjdGl2ZVxuJzsgZmkKICByZXR1cm4gIiRmYWlsdXJlcyIKfQpzdGF0dXMoKSB7CiAgbG9jYWwgZmFpbHVyZXM9MCBzZXJ2aWNlIHZlcnNpb249IiR7TUlTVEJPUk5fQk9PVFNUUkFQX1ZFUlNJT046LXVua25vd259IgogIHByaW50ZiAnTWlzdGJvcm4gaW5zdGFsbGF0aW9uXG4nCiAgcHJpbnRmICcgIElORk8gIGJvb3RzdHJhcCB2ZXJzaW9uOiAlc1xuJyAiJHZlcnNpb24iCiAgZm9yIHNlcnZpY2UgaW4gZG9ja2VyIHRhaWxzY2FsZSByY2xvbmUgdWZ3IGZhaWwyYmFuLWNsaWVudDsgZG8gcmVwb3J0X2NvbW1hbmRfdmVyc2lvbiAiJHNlcnZpY2UiOyBkb25lCiAgaWYgW1sgLXggIiRNSVNUQk9STl9SVU5ORVJfUEFUSCIgXV07IHRoZW4KICAgIHByaW50ZiAnICBQQVNTICBydW5uZXIgaW5zdGFsbGVkOiAlc1xuJyAiJE1JU1RCT1JOX1JVTk5FUl9QQVRIIgogIGVsc2UKICAgIHByaW50ZiAnICBGQUlMICBydW5uZXIgbWlzc2luZzogJXNcbicgIiRNSVNUQk9STl9SVU5ORVJfUEFUSCIKICAgIGZhaWx1cmVzPTEKICBmaQogIGlmIFtbIC1kICIkUlVOVElQSV9QQVRIIiBdXTsgdGhlbgogICAgcHJpbnRmICcgIFBBU1MgIFJ1bnRpcGkgZGlyZWN0b3J5OiAlc1xuJyAiJFJVTlRJUElfUEFUSCIKICBlbHNlCiAgICBwcmludGYgJyAgRkFJTCAgUnVudGlwaSBkaXJlY3RvcnkgbWlzc2luZzogJXNcbicgIiRSVU5USVBJX1BBVEgiCiAgICBmYWlsdXJlcz0xCiAgZmkKCiAgcHJpbnRmICdcblNlcnZpY2VzXG4nCiAgZm9yIHNlcnZpY2UgaW4gZG9ja2VyIHRhaWxzY2FsZWQgZmFpbDJiYW47IGRvCiAgICBpZiAhIGNvbW1hbmQgLXYgc3lzdGVtY3RsID4vZGV2L251bGwgMj4mMTsgdGhlbgogICAgICBwcmludGYgJyAgSU5GTyAgc3lzdGVtZCB1bmF2YWlsYWJsZTsgY2Fubm90IGluc3BlY3QgJXNcbicgIiRzZXJ2aWNlIgogICAgICBicmVhawogICAgZWxpZiBzeXN0ZW1jdGwgaXMtYWN0aXZlIC0tcXVpZXQgIiRzZXJ2aWNlIjsgdGhlbgogICAgICBwcmludGYgJyAgUEFTUyAgJXMgYWN0aXZlXG4nICIkc2VydmljZSIKICAgIGVsc2UKICAgICAgcHJpbnRmICcgIFdBUk4gICVzIGluYWN0aXZlXG4nICIkc2VydmljZSIKICAgIGZpCiAgZG9uZQogIGJvb3RzdHJhcF9zdGF0ZV9zdGF0dXMgfHwgZmFpbHVyZXM9MQogIGNvbmZpZ3VyYXRpb25fc3RhdHVzIHx8IGZhaWx1cmVzPTEKICByZXR1cm4gIiRmYWlsdXJlcyIKfQpmaXhfc2VydmljZXMoKSB7CiAgbG9jYWwgc2VydmljZSBjaGFuZ2VkPTAKICBbWyAiJEVVSUQiID09IDAgXV0gfHwgZGllICJydW4gJ21pc3Rib3JuIGZpeCcgd2l0aCBzdWRvIgogIGNvbW1hbmQgLXYgc3lzdGVtY3RsID4vZGV2L251bGwgMj4mMSB8fCBkaWUgInN5c3RlbWQgaXMgcmVxdWlyZWQgdG8gcmVwYWlyIHNlcnZpY2VzIgogIGZvciBzZXJ2aWNlIGluIGRvY2tlciB0YWlsc2NhbGVkOyBkbwogICAgaWYgISBjb21tYW5kIC12ICIkc2VydmljZSIgPi9kZXYvbnVsbCAyPiYxOyB0aGVuCiAgICAgIHByaW50ZiAnICBTS0lQICAlcyBpcyBub3QgaW5zdGFsbGVkXG4nICIkc2VydmljZSIKICAgICAgY29udGludWUKICAgIGZpCiAgICBpZiBzeXN0ZW1jdGwgaXMtYWN0aXZlIC0tcXVpZXQgIiRzZXJ2aWNlIiAmJiBzeXN0ZW1jdGwgaXMtZW5hYmxlZCAtLXF1aWV0ICIkc2VydmljZSI7IHRoZW4KICAgICAgcHJpbnRmICcgIFBBU1MgICVzIGlzIGFscmVhZHkgZW5hYmxlZCBhbmQgYWN0aXZlXG4nICIkc2VydmljZSIKICAgIGVsc2UKICAgICAgcHJpbnRmICcgIEZJWCAgIGVuYWJsaW5nIGFuZCBzdGFydGluZyAlc1xuJyAiJHNlcnZpY2UiCiAgICAgIHN5c3RlbWN0bCBlbmFibGUgLS1ub3cgIiRzZXJ2aWNlIgogICAgICBjaGFuZ2VkPTEKICAgIGZpCiAgZG9uZQogIGlmIFtbICIkY2hhbmdlZCIgPT0gMCBdXTsgdGhlbiBwcmludGYgJ0NvcmUgc2VydmljZXMgYXJlIGFscmVhZHkgaGVhbHRoeS5cbic7IGZpCiAgcHJpbnRmICdVRlcgYW5kIFNTSCBzZXR0aW5ncyBhcmUgbGVmdCB1bmNoYW5nZWQ7IHJldmlldyB0aGVtIHdpdGggbWlzdGJvcm4gc2VjdXJpdHktc3RhdHVzLlxuJwp9CnVwZGF0ZV9ydW50aXBpKCkgewogIHByaW50ZiAnVXBkYXRpbmcgUnVudGlwaSBjb3JlICh3aXRoIGFwcCBzbmFwc2hvdHMpLi4uXG4nCiAgdGFza19ldmVudCBjb3JlIHVwZGF0ZSBzdGFydGVkCiAgc25hcHNob3RfYXBwcyA8IDwoYXBwX3JlZnMpCiAgcnVuX3J1bnRpcGkgdXBkYXRlIGxhdGVzdAogIHRhc2tfZXZlbnQgY29yZSB1cGRhdGUgY29tcGxldGVkCiAgcHJpbnRmICdcblVwZGF0aW5nIGFwcCBzdG9yZXMuLi5cbicKICB0YXNrX2V2ZW50IGFwcHN0b3JlcyB1cGRhdGUgc3RhcnRlZAogIHJ1bl9ydW50aXBpIGFwcHN0b3JlIHVwZGF0ZQogIHRhc2tfZXZlbnQgYXBwc3RvcmVzIHVwZGF0ZSBjb21wbGV0ZWQKICBwcmludGYgJ1xuVXBkYXRpbmcgYXBwcyAod2l0aCBzbmFwc2hvdHMpLi4uXG4nCiAgdGFza19ldmVudCBhcHBzIHVwZGF0ZSBzdGFydGVkCiAgdXBkYXRlX2FwcHMKICB0YXNrX2V2ZW50IGFwcHMgdXBkYXRlIGNvbXBsZXRlZAogIHByaW50ZiAnXG5NaXN0Ym9ybiB1cGRhdGVzIGNvbXBsZXRlLlxuJwp9CnVwZGF0ZV9ib290c3RyYXAoKSB7CiAgW1sgIiRFVUlEIiA9PSAwIF1dIHx8IGRpZSAicnVuICdtaXN0Ym9ybiB1cGdyYWRlJyB3aXRoIHN1ZG8iCiAgbG9jYWwgbGF0ZXN0IGN1cnJlbnQgdGVtcF9kaXIgaW5zdGFsbGVyX3VybAogIHRhc2tfZXZlbnQgYm9vdHN0cmFwIHJlbGVhc2Ugc3RhcnRlZAogIHByaW50ZiAnQ2hlY2tpbmcgdGhlIGxhdGVzdCBzdGFibGUgTWlzdGJvcm4gQm9vdHN0cmFwIHJlbGVhc2UuLi5cbicKICBsYXRlc3Q9IiQoY3VybCAtZnNTTCBodHRwczovL2FwaS5naXRodWIuY29tL3JlcG9zL0Vja1BoaS9taXN0Ym9ybi1ib290c3RyYXAvcmVsZWFzZXMvbGF0ZXN0IHwgc2VkIC1uRSAncy9eW1s6c3BhY2U6XV0qInRhZ19uYW1lIjpbWzpzcGFjZTpdXSoiKHZbMC05XStcLlswLTldK1wuWzAtOV0rKSIuKi9cMS9wJyB8IGhlYWQgLW4xKSIgfHwgZGllICJjb3VsZCBub3QgY2hlY2sgR2l0SHViIHJlbGVhc2VzIgogIFtbICIkbGF0ZXN0IiA9fiBedlswLTldK1wuWzAtOV0rXC5bMC05XSskIF1dIHx8IGRpZSAiR2l0SHViIHJldHVybmVkIG5vIHN0YWJsZSBib290c3RyYXAgcmVsZWFzZSIKICB0YXNrX2V2ZW50IGJvb3RzdHJhcCByZWxlYXNlIGNvbXBsZXRlZAogIGN1cnJlbnQ9IiR7TUlTVEJPUk5fQk9PVFNUUkFQX1ZFUlNJT046LXYwLjAuMH0iCiAgY3VycmVudD0iJHtjdXJyZW50I3Z9IgogIGxhdGVzdD0iJHtsYXRlc3Qjdn0iCiAgaWYgW1sgIiQocHJpbnRmICclc1xuJXNcbicgIiRjdXJyZW50IiAiJGxhdGVzdCIgfCBzb3J0IC1WIHwgdGFpbCAtbjEpIiA9PSAiJGN1cnJlbnQiIF1dOyB0aGVuCiAgICBwcmludGYgJ01pc3Rib3JuIEJvb3RzdHJhcCAlcyBpcyBhbHJlYWR5IGN1cnJlbnQuXG4nICIkY3VycmVudCIKICAgIHRhc2tfZXZlbnQgYm9vdHN0cmFwIGluc3RhbGxlciBza2lwcGVkCiAgICByZXR1cm4gMAogIGZpCiAgaW5zdGFsbGVyX3VybD0iaHR0cHM6Ly9yYXcuZ2l0aHVidXNlcmNvbnRlbnQuY29tL0Vja1BoaS9taXN0Ym9ybi1ib290c3RyYXAvdiR7bGF0ZXN0fS9pbnN0YWxsLnNoIgogIHRlbXBfZGlyPSIkKG1rdGVtcCAtZCkiCiAgdHJhcCAncm0gLXJmICIkdGVtcF9kaXIiJyBSRVRVUk4KICB0YXNrX2V2ZW50IGJvb3RzdHJhcCBpbnN0YWxsZXIgc3RhcnRlZAogIHByaW50ZiAnVXBkYXRpbmcgTWlzdGJvcm4gQm9vdHN0cmFwICVzIOKGkiAlcy4uLlxuJyAiJGN1cnJlbnQiICIkbGF0ZXN0IgogIGN1cmwgLWZzU0wgIiRpbnN0YWxsZXJfdXJsIiAtbyAiJHRlbXBfZGlyL2luc3RhbGwuc2giIHx8IGRpZSAiY291bGQgbm90IGRvd25sb2FkIGJvb3RzdHJhcCBpbnN0YWxsZXIiCiAgTUlTVEJPUk5fVkVSU0lPTj0idiR7bGF0ZXN0fSIgTUlTVEJPUk5fVFVJPTAgYmFzaCAiJHRlbXBfZGlyL2luc3RhbGwuc2giIHNlcnZlciAtLXllcwogIHRhc2tfZXZlbnQgYm9vdHN0cmFwIGluc3RhbGxlciBjb21wbGV0ZWQKICBwcmludGYgJ01pc3Rib3JuIEJvb3RzdHJhcCB1cGRhdGVkIHRvICVzLlxuJyAiJGxhdGVzdCIKfQp1cGRhdGVfYXBwcygpIHsKICBsb2NhbCBiYWNrdXA9MSByZWZzPSgpIHJlZgogIFtbICIkezE6LX0iID09IC0tbm8tYmFja3VwIF1dICYmIHsgYmFja3VwPTA7IHNoaWZ0OyB9CiAgaWYgW1sgJCMgLWd0IDAgXV07IHRoZW4gcmVmcz0oIiRAIik7IGVsc2UgbWFwZmlsZSAtdCByZWZzIDwgPChhcHBfcmVmcyk7IGZpCiAgZm9yIHJlZiBpbiAiJHtyZWZzW0BdfSI7IGRvIFtbICIkYmFja3VwIiA9PSAxIF1dICYmIHJ1bl9ydW50aXBpIGFwcCBiYWNrdXAgIiRyZWYiOyBydW5fcnVudGlwaSBhcHAgdXBkYXRlICIkcmVmIjsgZG9uZQp9CnVzYWdlKCkgewogIGNhdCA8PCdFT0YnClVzYWdlOiBtaXN0Ym9ybiBDT01NQU5EIFtBUkdTXQogIHN0YXR1cyAgICAgICAgICAgICAgICAgICAgICAgdmVyaWZ5IGluc3RhbGxhdGlvbiwgdmVyc2lvbnMsIHN0YXRlIGFuZCBjb25maWd1cmF0aW9uCiAgZG9jdG9yICAgICAgICAgICAgICAgICAgICAgICBhdWRpdCBEb2NrZXIsIFJ1bnRpcGksIFRhaWxzY2FsZSwgcmNsb25lIGFuZCBzZWN1cml0eQogIHBsYW4gW1JFTUVESUFUSU9OXSAgICAgICAgICAgY29tcGFyZSBkZXNpcmVkIGNvbmZpZ3VyYXRpb24gd2l0aCBob3N0IHN0YXRlCiAgcmVjb25jaWxlIFtSRU1FRElBVElPTl0gICAgICBhcHBseSBhbiBhcHByb3ZlZCBob3N0IHJlbWVkaWF0aW9uCiAgZG9jdG9yIC0tZml4IFstLXNhZmVdICAgICAgICBwbGFuIGFuZCByZWNvbmNpbGUgdGhyb3VnaCB0aGUgc2FtZSBzYWZldHkgcG9saWN5CiAgZml4ICAgICAgICAgICAgICAgICAgICAgICAgICBlbmFibGUgYW5kIHN0YXJ0IGluc3RhbGxlZCBEb2NrZXIvVGFpbHNjYWxlIHNlcnZpY2VzCiAgc2VjdXJpdHktc3RhdHVzICAgICAgICAgICAgICBzaG93IFNTSCwgVUZXLCBmYWlsMmJhbiBhbmQgVGFpbHNjYWxlIHN0YXR1cwogIHRhaWxzY2FsZS1zdGF0dXMgICAgICAgICAgICAgc2hvdyBUYWlsc2NhbGUgc3RhdHVzCiAgcmNsb25lLWNvbmZpZyAgICAgICAgICAgICAgICBvcGVuIHJjbG9uZSdzIGNvbmZpZ3VyYXRpb24gVUkKICB1cGRhdGUtYXBwcyBbLS1uby1iYWNrdXBdIFtBUFA6U1RPUkUgLi4uXQogIHVwZGF0ZS1jb3JlIFstLW5vLWJhY2t1cF0gW1ZFUlNJT05dCiAgdXBkYXRlLWFwcHN0b3JlcwogIHVwZ3JhZGUgICAgICAgICAgICAgICAgICAgICAgdXBncmFkZSBNaXN0Ym9ybiBCb290c3RyYXAgdG8gdGhlIGxhdGVzdCBzdGFibGUgcmVsZWFzZQogIHVwZGF0ZSAgICAgICAgICAgICAgICAgICAgICAgYWxpYXMgZm9yIHVwZ3JhZGUKICB1cGRhdGUtcnVudGlwaSAgICAgICAgICAgICAgIHVwZGF0ZSBSdW50aXBpIGNvcmUsIGFwcCBzdG9yZXMgYW5kIGFwcHMgKHdpdGggYmFja3VwcykKRU9GCn0KY2FzZSAiJHsxOi19IiBpbgogIGhlbHB8LWh8LS1oZWxwfCcnKSB1c2FnZSA7OwogIHN0YXR1cykgc3RhdHVzIDs7CiAgZG9jdG9yKSBkb2N0b3IgOzsKICBmaXgpIGZpeF9zZXJ2aWNlcyA7OwogIHNlY3VyaXR5LXN0YXR1cykgc3NoZCAtVCAyPi9kZXYvbnVsbCB8IGdyZXAgLUUgJ3Bhc3N3b3JkYXV0aGVudGljYXRpb258cGVybWl0cm9vdGxvZ2lufF5wb3J0JzsgdWZ3IHN0YXR1cyB2ZXJib3NlOyBmYWlsMmJhbi1jbGllbnQgc3RhdHVzIHNzaGQgfHwgdHJ1ZTsgdGFpbHNjYWxlIHN0YXR1cyB8fCB0cnVlIDs7CiAgdGFpbHNjYWxlLXN0YXR1cykgdGFpbHNjYWxlIHN0YXR1cyA7OwogIHJjbG9uZS1jb25maWcpIHJjbG9uZSBjb25maWcgOzsKICB1cGRhdGUtYXBwcykgc2hpZnQ7IHVwZGF0ZV9hcHBzICIkQCIgOzsKICB1cGRhdGUtY29yZSkgc2hpZnQ7IGJhY2t1cD0xOyBbWyAiJHsxOi19IiA9PSAtLW5vLWJhY2t1cCBdXSAmJiB7IGJhY2t1cD0wOyBzaGlmdDsgfTsgW1sgIiRiYWNrdXAiID09IDEgXV0gJiYgc25hcHNob3RfYXBwcyA8IDwoYXBwX3JlZnMpOyBydW5fcnVudGlwaSB1cGRhdGUgIiR7MTotbGF0ZXN0fSIgOzsKICB1cGRhdGUtYXBwc3RvcmVzKSBydW5fcnVudGlwaSBhcHBzdG9yZSB1cGRhdGUgOzsKICB1cGdyYWRlfHVwZGF0ZSkgdXBkYXRlX2Jvb3RzdHJhcCA7OwogIHVwZGF0ZS1ydW50aXBpKSB1cGRhdGVfcnVudGlwaSA7OwogICopIGRpZSAidW5rbm93biBjb21tYW5kOiAkMSIgOzsKZXNhYwo='
export MISTBORN_UPDATE_PLAN_B64='dmVyc2lvbiA9IDIKY29sbGVjdGlvbiA9ICJ1cGRhdGUiCgpbW3N0YWdlc11dCmlkID0gImJvb3RzdHJhcCIKdGl0bGUgPSAiTWlzdGJvcm4gQm9vdHN0cmFwIgpoZWxwID0gIkNoZWNrcyB0aGUgbGF0ZXN0IHN0YWJsZSByZWxlYXNlIGFuZCByZXJ1bnMgaXRzIGluc3RhbGxlciB0byByZWZyZXNoIHRoZSBNaXN0Ym9ybiB0b29sIHdoaWxlIHJlc3VtaW5nIGNvbXBsZXRlZCBzZXR1cCBzdGFnZXMuIgpbW3N0YWdlcy50YXNrc11dCmlkID0gInJlbGVhc2UiCnRpdGxlID0gIkNoZWNrIGxhdGVzdCBzdGFibGUgYm9vdHN0cmFwIHJlbGVhc2UiCmFjdGlvbiA9ICJHaXRIdWIgcmVsZWFzZXMvbGF0ZXN0Igp3ZWlnaHQgPSAxCltbc3RhZ2VzLnRhc2tzXV0KaWQgPSAiaW5zdGFsbGVyIgp0aXRsZSA9ICJSZWZyZXNoIE1pc3Rib3JuIHJ1bm5lciBhbmQgY29tbWFuZCIKYWN0aW9uID0gInJ1biB0YWdnZWQgc2VydmVyIGluc3RhbGxlciAocmVzdW1lIGNvbXBsZXRlZCBzdGFnZXMpIgp3ZWlnaHQgPSAzCg=='
export MISTBORN_CONFIG_B64='IyBNaXN0Ym9ybiBkZXNpcmVkLXN0YXRlIGNvbmZpZ3VyYXRpb24uIE9wdGlvbmFsIHNlY3Rpb25zIGFyZSBvbWl0dGVkIHNvIHRoZQojIGhvc3QgcmVtYWlucyB1bm1hbmFnZWQgdW50aWwgYW4gYWRtaW5pc3RyYXRvciBleHBsaWNpdGx5IGFkb3B0cyBhIHBvbGljeS4KdmVyc2lvbiA9IDEKcHJvZmlsZSA9ICJ2cHMiCg=='
export MISTBORN_CONFIG_EXAMPLE_B64='dmVyc2lvbiA9IDEKcHJvZmlsZSA9ICJ2cHMiCgpbc3NoXQpwb3J0ID0gMjIKcGFzc3dvcmRfYXV0aGVudGljYXRpb24gPSBmYWxzZQpyb290X2xvZ2luID0gZmFsc2UKCltmaXJld2FsbF0KZW5hYmxlZCA9IHRydWUKZGVmYXVsdF9pbmNvbWluZyA9ICJkZW55IgpwdWJsaWNfdGNwX3BvcnRzID0gWzgwLCA0NDNdCgpbZmlyZXdhbGwucGxleF0KZW5hYmxlZCA9IHRydWUKcHVibGljX3JlbW90ZV9hY2Nlc3MgPSB0cnVlCmxhbl9jaWRyID0gIjE5Mi4xNjguMS4wLzI0Igp0YWlsc2NhbGUgPSB0cnVlCgpbdGFpbHNjYWxlXQpzc2ggPSB0cnVlCmFkdmVydGlzZV9leGl0X25vZGUgPSBmYWxzZQphdXRvX3VwZGF0ZSA9IHRydWUK'
# shellcheck shell=bash

ui_is_terminal() { [[ -t 1 && -z "${NO_COLOR:-}" ]]; }

ui_color() {
  local code="$1"
  if ui_is_terminal; then printf '\033[%sm' "$code"; fi
}

ui_reset() { ui_color 0; }
ui_header() { printf '\n%s%s%s\n\n' "$(ui_color '1;36')" "$1" "$(ui_reset)"; }
ui_info() { printf '  %s•%s %s\n' "$(ui_color 36)" "$(ui_reset)" "$1"; }
ui_success() { printf '  %s✓%s %s\n' "$(ui_color 32)" "$(ui_reset)" "$1"; }
ui_warn() { printf '  %s!%s %s\n' "$(ui_color 33)" "$(ui_reset)" "$1" >&2; }
ui_error() { printf '  %s✗%s %s\n' "$(ui_color 31)" "$(ui_reset)" "$1" >&2; }
ui_step() { printf '\n%s==>%s %s\n' "$(ui_color '1;34')" "$(ui_reset)" "$1"; }

mistborn_task_event() {
  local stage="$1" task="$2" state="$3"
  [[ -n "${MISTBORN_PROGRESS_FILE:-}" ]] || return 0
  printf '%s\t%s\t%s\n' "$stage" "$task" "$state" >>"$MISTBORN_PROGRESS_FILE" || true
}
mistborn_task_start() { mistborn_task_event "${MISTBORN_PROGRESS_STAGE:-}" "$1" started; }
mistborn_task_complete() { mistborn_task_event "${MISTBORN_PROGRESS_STAGE:-}" "$1" completed; }
mistborn_task_skip() { mistborn_task_event "${MISTBORN_PROGRESS_STAGE:-}" "$1" skipped; }

mistborn_task_selected() {
  local task="$1"
  [[ -z "${MISTBORN_TASKS:-}" || ",${MISTBORN_TASKS}," == *",${task},"* ]]
}

ui_confirm() {
  local prompt="$1" reply
  [[ "${MISTBORN_YES:-0}" == 1 ]] && return 0
  [[ -t 0 ]] || return 1
  read -r -p "$prompt [y/N] " reply
  [[ "$reply" =~ ^[Yy]$ ]]
}
# shellcheck shell=bash

mistborn_run() {
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    printf '  [dry-run]'; printf ' %q' "$@"; printf '\n'
    return 0
  fi
  "$@"
}

mistborn_has_terminal() {
  [[ -c /dev/tty ]] && ( : </dev/tty ) 2>/dev/null
}

mistborn_run_interactive() {
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    printf '  [dry-run]'; printf ' %q' "$@"; printf '\n'
    return 0
  fi
  if [[ "${MISTBORN_EMBEDDED_TERMINAL:-0}" == 1 ]]; then
    "$@"
    return
  fi
  if ! mistborn_has_terminal; then
    ui_error "This step requires a terminal. Run the downloaded installer directly or use its non-interactive option."
    return 1
  fi
  "$@" </dev/tty
}

mistborn_require_root() {
  [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]] && return 0
  if [[ "$EUID" -ne 0 ]]; then
    ui_error "Run this installer through sudo."
    return 1
  fi
}

mistborn_target_user() {
  if [[ -n "${MISTBORN_USER:-}" ]]; then
    printf '%s\n' "$MISTBORN_USER"
  elif [[ -n "${SUDO_USER:-}" && "$SUDO_USER" != root ]]; then
    printf '%s\n' "$SUDO_USER"
  else
    printf '%s\n' root
  fi
}

mistborn_user_home() {
  if command -v getent >/dev/null 2>&1; then
    getent passwd "$1" | cut -d: -f6
  else
    awk -F: -v user="$1" '$1 == user { print $6 }' /etc/passwd
  fi
}

mistborn_apt_install() {
  DEBIAN_FRONTEND=noninteractive mistborn_run apt-get install -y --no-install-recommends "$@"
}
# shellcheck shell=bash

module_common_description="System prerequisites"

module_common_apply() {
  ui_step "$module_common_description"
  mistborn_require_root
  if mistborn_task_selected apt-index; then
    mistborn_task_start apt-index; mistborn_run apt-get update; mistborn_task_complete apt-index
  fi
  if mistborn_task_selected base-packages; then
    mistborn_task_start base-packages; mistborn_apt_install ca-certificates curl git; mistborn_task_complete base-packages
  fi
  ui_success "$module_common_description"
}
# shellcheck shell=bash

module_docker_description="Docker Engine"

module_docker_apply() {
  ui_step "$module_docker_description"
  if mistborn_task_selected engine; then
  mistborn_task_start engine
  if command -v docker >/dev/null 2>&1; then
    ui_info "Docker already installed"
  else
    mistborn_apt_install docker.io
  fi
  mistborn_task_complete engine
  fi
  if mistborn_task_selected service; then
    mistborn_task_start service; mistborn_run systemctl enable --now docker; mistborn_task_complete service
  fi
  ui_success "$module_docker_description"
}
# shellcheck shell=bash

module_zsh_description="Zsh, Oh My Zsh, and Powerlevel10k"

module_zsh_apply() {
  local user home custom_dir zshrc
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }

  ui_step "$module_zsh_description for $user"
  if mistborn_task_selected packages; then
  mistborn_task_start packages
  mistborn_apt_install zsh git
  mistborn_task_complete packages
  fi
  custom_dir="$home/.oh-my-zsh"
  if mistborn_task_selected oh-my-zsh; then
  mistborn_task_start oh-my-zsh
  if [[ ! -d "$custom_dir/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/ohmyzsh/ohmyzsh.git "$custom_dir"
  else
    ui_info "Oh My Zsh already installed"
  fi
  mistborn_task_complete oh-my-zsh
  fi
  if mistborn_task_selected powerlevel10k; then
  mistborn_task_start powerlevel10k
  if [[ ! -d "$custom_dir/custom/themes/powerlevel10k/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/romkatv/powerlevel10k.git \
      "$custom_dir/custom/themes/powerlevel10k"
  else
    ui_info "Powerlevel10k already installed"
  fi
  mistborn_task_complete powerlevel10k
  fi

  if mistborn_task_selected configuration; then
  mistborn_task_start configuration
  zshrc="$home/.zshrc"
  if [[ -f "$zshrc" && ! -f "$zshrc.mistborn-backup" ]]; then
    mistborn_run cp -a "$zshrc" "$zshrc.mistborn-backup"
  fi
  if [[ ! -f "$zshrc" || "${MISTBORN_REPLACE_ZSHRC:-0}" == 1 ]]; then
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would write $zshrc"
    else
      # These variables must expand when Zsh starts, not during installation.
      # shellcheck disable=SC2016
      printf '%s\n' 'export ZSH="$HOME/.oh-my-zsh"' 'ZSH_THEME="powerlevel10k/powerlevel10k"' \
        'plugins=(git sudo)' 'source "$ZSH/oh-my-zsh.sh"' '[[ -f ~/.p10k.zsh ]] && source ~/.p10k.zsh' >"$zshrc"
      chown "$user":"$(id -gn "$user")" "$zshrc"
    fi
  else
    ui_warn "Keeping existing $zshrc; set MISTBORN_REPLACE_ZSHRC=1 to replace it"
  fi
  mistborn_task_complete configuration
  mistborn_run chsh -s "$(command -v zsh)" "$user"
  fi
  ui_success "$module_zsh_description"
}
# shellcheck shell=bash

module_tailscale_description="Tailscale"

module_tailscale_apply() {
  local sysctl_file=/etc/sysctl.d/99-mistborn-tailscale.conf
  ui_step "$module_tailscale_description"
  if mistborn_task_selected install; then
  mistborn_task_start install
  if command -v tailscale >/dev/null 2>&1; then
    ui_info "Tailscale already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install Tailscale from packages.tailscale.com"
  else
    curl -fsSL https://tailscale.com/install.sh | sh
  fi
  mistborn_task_complete install
  fi
  if mistborn_task_selected forwarding && [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]]; then
    mistborn_task_start forwarding
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would enable persistent IPv4 and IPv6 forwarding in $sysctl_file"
    else
      printf '%s\n' 'net.ipv4.ip_forward = 1' 'net.ipv6.conf.all.forwarding = 1' >"$sysctl_file"
      sysctl -p "$sysctl_file"
    fi
    mistborn_task_complete forwarding
  elif mistborn_task_selected forwarding; then
    mistborn_task_skip forwarding
  fi
  local args=(up)
  [[ "${MISTBORN_TAILSCALE_SSH:-0}" == 1 ]] && args+=(--ssh)
  [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]] && args+=(--advertise-exit-node)
  if mistborn_task_selected connect; then
  mistborn_task_start connect
  if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
    args+=(--auth-key "$TAILSCALE_AUTH_KEY")
    mistborn_run tailscale "${args[@]}"
  else
    mistborn_run_interactive tailscale "${args[@]}"
  fi
  mistborn_task_complete connect
  fi
  if mistborn_task_selected auto-update && [[ "${MISTBORN_TAILSCALE_AUTO_UPDATE:-0}" == 1 ]]; then
    mistborn_task_start auto-update
    mistborn_run tailscale set --auto-update
    mistborn_task_complete auto-update
  elif mistborn_task_selected auto-update; then
    mistborn_task_skip auto-update
  fi
  ui_success "$module_tailscale_description"
}
# shellcheck shell=bash

module_runtipi_description="Runtipi"

module_runtipi_apply() {
  ui_step "$module_runtipi_description"
  mistborn_task_selected install || return 0
  mistborn_task_start install
  if command -v runtipi-cli >/dev/null 2>&1 || [[ -x /opt/runtipi/runtipi-cli ]]; then
    ui_info "Runtipi already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would run the official Runtipi installer"
  else
    curl -fsSL https://setup.runtipi.io | bash
  fi
  mistborn_task_complete install
  ui_success "$module_runtipi_description"
}
# shellcheck shell=bash

module_rclone_description="rclone"

module_rclone_apply() {
  local user home
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }
  ui_step "$module_rclone_description"
  if mistborn_task_selected package; then
    mistborn_task_start package; mistborn_apt_install rclone; mistborn_task_complete package
  fi
  if mistborn_task_selected configuration && [[ "${MISTBORN_RCLONE_CONFIGURE:-0}" == 1 ]]; then
    mistborn_task_start configuration
    if [[ "$user" == root ]]; then
      mistborn_run_interactive env HOME="$home" rclone config
    else
      mistborn_run_interactive runuser -u "$user" -- env HOME="$home" rclone config
    fi
    mistborn_task_complete configuration
  elif mistborn_task_selected configuration; then
    mistborn_task_skip configuration
  fi
  ui_success "$module_rclone_description"
}
# shellcheck shell=bash

module_security_description="SSH, UFW, and fail2ban hardening"

mistborn_valid_ipv4_cidr() {
  local cidr="$1" address prefix octet
  local -a octets
  [[ "$cidr" == */* ]] || return 1
  address="${cidr%/*}"
  prefix="${cidr##*/}"
  [[ "$prefix" =~ ^[0-9]+$ ]] && ((10#$prefix <= 32)) || return 1
  IFS=. read -r -a octets <<<"$address"
  [[ "${#octets[@]}" -eq 4 ]] || return 1
  for octet in "${octets[@]}"; do
    [[ "$octet" =~ ^[0-9]+$ ]] && ((10#$octet <= 255)) || return 1
  done
}

mistborn_configure_plex_ufw() {
  local profile=/etc/ufw/applications.d/plexmediaserver temporary lan_cidr
  lan_cidr="${MISTBORN_PLEX_LAN_CIDR:-}"
  if [[ -n "$lan_cidr" ]] && ! mistborn_valid_ipv4_cidr "$lan_cidr"; then
    ui_error "MISTBORN_PLEX_LAN_CIDR must be an IPv4 CIDR such as 192.168.1.0/24"
    return 1
  fi

  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install the Plex UFW application profiles at $profile"
  else
    temporary="$(mktemp)"
    printf '%s\n' \
      '[plexmediaserver]' \
      'title=Plex Media Server (Standard)' \
      'description=The Plex Media Server' \
      'ports=32400/tcp|3005/tcp|5353/udp|8324/tcp|32410:32414/udp' \
      '' \
      '[plexmediaserver-dlna]' \
      'title=Plex Media Server (DLNA)' \
      'description=The Plex Media Server (additional DLNA capability only)' \
      'ports=1900/udp|32469/tcp' \
      '' \
      '[plexmediaserver-all]' \
      'title=Plex Media Server (Standard + DLNA)' \
      'description=The Plex Media Server (with additional DLNA capability)' \
      'ports=32400/tcp|3005/tcp|5353/udp|8324/tcp|32410:32414/udp|1900/udp|32469/tcp' \
      >"$temporary"
    if install -m 0644 "$temporary" "$profile"; then
      rm -f -- "$temporary"
    else
      rm -f -- "$temporary"
      return 1
    fi
  fi

  mistborn_run ufw app update plexmediaserver
  mistborn_run ufw allow 32400/tcp comment 'Plex remote access'
  if [[ -n "$lan_cidr" ]]; then
    mistborn_run ufw allow from "$lan_cidr" to any app plexmediaserver-all
  fi
  if [[ "${MISTBORN_PLEX_TAILSCALE:-0}" == 1 ]]; then
    mistborn_run ufw allow in on tailscale0 to any app plexmediaserver-all
  fi
}

module_security_apply() {
  local ssh_port="${MISTBORN_SSH_PORT:-22}" ssh_config=/etc/ssh/sshd_config backup
  ui_step "$module_security_description"
  if [[ "${MISTBORN_HARDEN:-0}" != 1 ]]; then
    ui_warn "Security hardening is opt-in; re-run with MISTBORN_HARDEN=1 after testing SSH keys"
    for task in packages ssh firewall plex-firewall fail2ban tailscale-only; do
      mistborn_task_selected "$task" && mistborn_task_skip "$task"
    done
    return 0
  fi
  if mistborn_task_selected ssh && [[ "${MISTBORN_DISABLE_PASSWORD_AUTH:-1}" == 1 && "${MISTBORN_DRY_RUN:-0}" != 1 ]]; then
    local user home
    user="$(mistborn_target_user)"; home="$(mistborn_user_home "$user")"
    if [[ ! -s "$home/.ssh/authorized_keys" && ! -s /root/.ssh/authorized_keys && "${MISTBORN_FORCE_SSH:-0}" != 1 ]]; then
      ui_error "No authorized_keys found; refusing to disable password authentication"
      return 1
    fi
  fi
  if mistborn_task_selected packages; then
    mistborn_task_start packages; mistborn_apt_install ufw fail2ban; mistborn_task_complete packages
  fi
  if mistborn_task_selected ssh; then
  mistborn_task_start ssh
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would harden $ssh_config and validate it before restart"
  else
    backup="${ssh_config}.bak-mistborn"
    [[ -e "$backup" ]] || cp -a "$ssh_config" "$backup"
    sed -i -E 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication no/' "$ssh_config"
    sed -i -E 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin no/' "$ssh_config"
    if grep -Eq '^[#[:space:]]*Port[[:space:]]+' "$ssh_config"; then
      sed -i -E "s/^[#[:space:]]*Port[[:space:]]+.*/Port $ssh_port/" "$ssh_config"
    else
      printf '\nPort %s\n' "$ssh_port" >>"$ssh_config"
    fi
    if ! sshd -t; then cp -a "$backup" "$ssh_config"; ui_error "Invalid sshd configuration; restored backup"; return 1; fi
    systemctl restart sshd
    install -m 0644 /dev/null /etc/fail2ban/jail.local
    printf '[sshd]\nenabled = true\nport = ssh\nmaxretry = %s\nbantime = %s\n' \
      "${MISTBORN_FAIL2BAN_MAXRETRY:-3}" "${MISTBORN_FAIL2BAN_BANTIME:-3600}" >/etc/fail2ban/jail.local
  fi
  mistborn_task_complete ssh
  fi
  if mistborn_task_selected firewall; then
  mistborn_task_start firewall
  mistborn_run ufw allow "$ssh_port/tcp"
  for port in ${MISTBORN_ALLOWED_TCP_PORTS:-}; do mistborn_run ufw allow "$port/tcp"; done
  mistborn_run ufw default deny incoming
  mistborn_run ufw --force enable
  mistborn_task_complete firewall
  fi
  if mistborn_task_selected plex-firewall && [[ "${MISTBORN_PLEX_UFW:-0}" == 1 ]]; then
    command -v ufw >/dev/null 2>&1 || mistborn_apt_install ufw
    mistborn_task_start plex-firewall
    mistborn_configure_plex_ufw
    mistborn_task_complete plex-firewall
  elif mistborn_task_selected plex-firewall; then
    mistborn_task_skip plex-firewall
  fi
  if mistborn_task_selected fail2ban; then
    mistborn_task_start fail2ban; mistborn_run systemctl enable --now fail2ban; mistborn_task_complete fail2ban
  fi
  if mistborn_task_selected tailscale-only && [[ "${MISTBORN_TAILSCALE_ONLY:-0}" == 1 ]]; then
    mistborn_task_start tailscale-only
    mistborn_run tailscale set --ssh=true
    mistborn_run ufw allow in on tailscale0
    mistborn_run ufw allow "${MISTBORN_TAILSCALE_PORT:-41641}/udp"
    mistborn_run ufw delete allow "$ssh_port/tcp" || true
    mistborn_task_complete tailscale-only
    ui_warn "Confirm a new Tailscale SSH session before disconnecting"
  elif mistborn_task_selected tailscale-only; then
    mistborn_task_skip tailscale-only
  fi
  ui_success "$module_security_description"
}
# shellcheck shell=bash

module_toolset_description="Mistborn host-management commands"

mistborn_config_task_applied() {
  [[ ",${MISTBORN_CONFIG_APPLIED_TASKS:-}," == *",$1,"* ]]
}

mistborn_config_bool() {
  local value="${1:-0}"
  [[ "$value" == 0 || "$value" == 1 ]] || return 1
  [[ "$value" == 1 ]] && printf 'true' || printf 'false'
}

mistborn_render_desired_config() {
  local output="$1" port first value
  printf '%s' "$MISTBORN_CONFIG_B64" | base64 -d >"$output" || return 1
  [[ "${MISTBORN_CONFIG_ADOPTION_ALLOWED:-0}" == 1 ]] || return 0

  if [[ "${MISTBORN_HARDEN+x}" == x && "${MISTBORN_HARDEN:-0}" == 1 ]] \
    && mistborn_config_task_applied security/ssh; then
    port="${MISTBORN_SSH_PORT:-22}"
    [[ "$port" =~ ^[0-9]+$ ]] && ((10#$port >= 1 && 10#$port <= 65535)) || return 1
    printf '\n[ssh]\nport = %s\npassword_authentication = false\nroot_login = false\n' "$port" >>"$output"
  fi

  if [[ "${MISTBORN_HARDEN+x}" == x && "${MISTBORN_HARDEN:-0}" == 1 ]] \
    && mistborn_config_task_applied security/firewall; then
    printf '\n[firewall]\nenabled = true\ndefault_incoming = "deny"\npublic_tcp_ports = [' >>"$output"
    first=1
    for value in ${MISTBORN_ALLOWED_TCP_PORTS:-}; do
      [[ "$value" =~ ^[0-9]+$ ]] && ((10#$value >= 1 && 10#$value <= 65535)) || return 1
      [[ "$first" == 1 ]] || printf ', ' >>"$output"
      printf '%s' "$value" >>"$output"
      first=0
    done
    printf ']\n' >>"$output"
    if [[ "${MISTBORN_PLEX_UFW+x}" == x && "${MISTBORN_PLEX_UFW:-0}" == 1 ]] \
      && mistborn_config_task_applied security/plex-firewall; then
      printf '\n[firewall.plex]\nenabled = true\npublic_remote_access = true\n' >>"$output"
      if [[ -n "${MISTBORN_PLEX_LAN_CIDR:-}" ]]; then
        printf 'lan_cidr = "%s"\n' "$MISTBORN_PLEX_LAN_CIDR" >>"$output"
      fi
      printf 'tailscale = %s\n' "$(mistborn_config_bool "${MISTBORN_PLEX_TAILSCALE:-0}")" >>"$output" || return 1
    fi
  fi

  if mistborn_config_task_applied tailscale/connect \
    && [[ "${MISTBORN_TAILSCALE_SSH+x}${MISTBORN_TAILSCALE_EXIT_NODE+x}${MISTBORN_TAILSCALE_AUTO_UPDATE+x}" == *x* ]]; then
    printf '\n[tailscale]\nssh = %s\nadvertise_exit_node = %s\nauto_update = %s\n' \
      "$(mistborn_config_bool "${MISTBORN_TAILSCALE_SSH:-0}")" \
      "$(mistborn_config_bool "${MISTBORN_TAILSCALE_EXIT_NODE:-0}")" \
      "$(mistborn_config_bool "${MISTBORN_TAILSCALE_AUTO_UPDATE:-0}")" >>"$output" || return 1
  fi
}

mistborn_publish_desired_config() {
  local etc_dir="$1" share_dir="$2" validator="$3" staging_dir config_staging owner
  install -d -m 0755 "$etc_dir" "$share_dir"
  staging_dir="$(mktemp -d "$etc_dir/.install.XXXXXX")"
  config_staging="$staging_dir/config.toml"
  owner="${MISTBORN_CONFIG_OWNER:-root:root}"
  (
    set -Ee
    trap 'rm -f -- "$config_staging" "$staging_dir/config.toml.example"; rmdir "$staging_dir" 2>/dev/null || true' EXIT
    mistborn_render_desired_config "$config_staging" || exit 1
    printf '%s' "$MISTBORN_CONFIG_EXAMPLE_B64" | base64 -d >"$staging_dir/config.toml.example" || exit 1
    "$validator" validate-config "$config_staging" || exit 1
    "$validator" validate-config "$staging_dir/config.toml.example" || exit 1
    chmod 0644 "$config_staging" "$staging_dir/config.toml.example" || exit 1
    chown "$owner" "$config_staging" "$staging_dir/config.toml.example" || exit 1
    if [[ "${MISTBORN_SKIP_FSYNC:-0}" != 1 ]]; then
      sync -f "$config_staging" || exit 1
      sync -f "$staging_dir/config.toml.example" || exit 1
    fi
    mv -f "$staging_dir/config.toml.example" "$share_dir/config.toml.example" || exit 1
    if [[ ! -e "$etc_dir/config.toml" ]]; then
      mv "$config_staging" "$etc_dir/config.toml" || exit 1
    fi
    if [[ "${MISTBORN_SKIP_FSYNC:-0}" != 1 ]]; then
      sync -f "$etc_dir" || exit 1
      sync -f "$share_dir" || exit 1
    fi
  )
}

module_toolset_apply() {
  ui_step "$module_toolset_description"
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install /usr/local/bin/mistborn and its Ratatui runner"
  else
    install -d -m 0755 /usr/local/lib/mistborn
    if mistborn_task_selected runner; then
    mistborn_task_start runner
    if [[ -n "${MISTBORN_RUNNER_BINARY:-}" && -x "$MISTBORN_RUNNER_BINARY" && "$MISTBORN_RUNNER_BINARY" != /usr/local/bin/mistborn-bootstrap ]]; then
      install -m 0755 "$MISTBORN_RUNNER_BINARY" /usr/local/bin/mistborn-bootstrap
      mistborn_task_complete runner
    else
      mistborn_task_skip runner
    fi
    fi
    if mistborn_task_selected command; then
    mistborn_task_start command
    install -d -m 0755 /usr/local/lib/mistborn/plans
    local staging_dir
    staging_dir="$(mktemp -d /usr/local/lib/mistborn/.install.XXXXXX)"
    printf '%s' "$MISTBORN_TOOL_B64" | base64 -d >"$staging_dir/host.sh"
    chmod 0644 "$staging_dir/host.sh"
    printf '%s' "$MISTBORN_UPDATE_PLAN_B64" | base64 -d >"$staging_dir/update.toml"
    chmod 0644 "$staging_dir/update.toml"
    mv -f "$staging_dir/host.sh" /usr/local/lib/mistborn/host.sh
    mv -f "$staging_dir/update.toml" /usr/local/lib/mistborn/plans/update.toml
    rmdir "$staging_dir"
    mistborn_publish_desired_config /etc/mistborn /usr/local/share/mistborn "${MISTBORN_RUNNER_BINARY:-/usr/local/bin/mistborn-bootstrap}"
    cat >/usr/local/bin/mistborn <<'MISTBORN_LAUNCHER'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ -x /usr/local/bin/mistborn-bootstrap ]]; then
  exec /usr/local/bin/mistborn-bootstrap host --script /usr/local/lib/mistborn/host.sh "$@"
fi
exec bash /usr/local/lib/mistborn/host.sh "$@"
MISTBORN_LAUNCHER
    chmod 0755 /usr/local/bin/mistborn
    mistborn_task_complete command
    fi
  fi
  ui_success "$module_toolset_description"
}
main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run) MISTBORN_DRY_RUN=1 ;;
      --yes) MISTBORN_YES=1 ;;
      --user) shift; MISTBORN_USER="${1:?--user requires a value}" ;;
      --only) shift; MISTBORN_ONLY="${1:?--only requires a module name}" ;;
      --tasks) shift; MISTBORN_TASKS="${1:?--tasks requires a comma-separated task list}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME] [--only MODULE] [--tasks IDS]\n' "$0"; return ;;
      *) ui_error "Unknown argument: $1"; return 2 ;;
    esac
    shift
  done
  if [[ -n "$MISTBORN_ONLY" ]]; then
    local known=0 module
    for module in "${MISTBORN_MODULES[@]}"; do
      [[ "$module" == "$MISTBORN_ONLY" ]] && known=1
    done
    [[ "$known" == 1 ]] || { ui_error "Unknown module: $MISTBORN_ONLY"; return 2; }
  fi
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Mistborn server setup"
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != common ]] || module_common_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != docker ]] || module_docker_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != zsh ]] || module_zsh_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != tailscale ]] || module_tailscale_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != runtipi ]] || module_runtipi_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != rclone ]] || module_rclone_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != security ]] || module_security_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != toolset ]] || module_toolset_apply
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Setup complete"
}
main "$@"
