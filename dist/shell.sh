#!/usr/bin/env bash
set -Eeuo pipefail
trap 'ui_error "Setup failed on line $LINENO"' ERR
MISTBORN_DRY_RUN=0
MISTBORN_YES=0
MISTBORN_ONLY=""
MISTBORN_MODULES=( common zsh )
export MISTBORN_TOOL_B64='IyEvdXNyL2Jpbi9lbnYgYmFzaApzZXQgLUVldW8gcGlwZWZhaWwKClJVTlRJUElfUEFUSD0iJHtSVU5USVBJX1BBVEg6LS9vcHQvcnVudGlwaX0iCgpkaWUoKSB7IHByaW50ZiAnZXJyb3I6ICVzXG4nICIkKiIgPiYyOyBleGl0IDE7IH0KdGFza19ldmVudCgpIHsKICBbWyAtbiAiJHtNSVNUQk9STl9QUk9HUkVTU19GSUxFOi19IiBdXSB8fCByZXR1cm4gMAogIHByaW50ZiAnJXNcdCVzXHQlc1xuJyAiJDEiICIkMiIgIiQzIiA+PiIkTUlTVEJPUk5fUFJPR1JFU1NfRklMRSIgfHwgdHJ1ZQp9CnJ1bnRpcGlfY2xpKCkgewogIGlmIFtbIC14ICIkUlVOVElQSV9QQVRIL3J1bnRpcGktY2xpIiBdXTsgdGhlbiBwcmludGYgJyVzXG4nICIkUlVOVElQSV9QQVRIL3J1bnRpcGktY2xpIgogIGVsaWYgY29tbWFuZCAtdiBydW50aXBpLWNsaSA+L2Rldi9udWxsOyB0aGVuIGNvbW1hbmQgLXYgcnVudGlwaS1jbGkKICBlbHNlIGRpZSAicnVudGlwaS1jbGkgbm90IGZvdW5kIHVuZGVyICRSVU5USVBJX1BBVEggb3IgUEFUSCI7IGZpCn0KcnVuX3J1bnRpcGkoKSB7ICIkKHJ1bnRpcGlfY2xpKSIgIiRAIjsgfQphcHBfcmVmcygpIHsKICBsb2NhbCBzdG9yZSBhcHAKICBmb3Igc3RvcmUgaW4gIiRSVU5USVBJX1BBVEgiL2FwcHMvKjsgZG8KICAgIFtbIC1kICIkc3RvcmUiIF1dIHx8IGNvbnRpbnVlCiAgICBmb3IgYXBwIGluICIkc3RvcmUiLyo7IGRvIFtbIC1kICIkYXBwIiBdXSAmJiBwcmludGYgJyVzOiVzXG4nICIkKGJhc2VuYW1lICIkYXBwIikiICIkKGJhc2VuYW1lICIkc3RvcmUiKSI7IGRvbmUKICBkb25lCn0Kc25hcHNob3RfYXBwcygpIHsgbG9jYWwgcmVmOyB3aGlsZSBJRlM9IHJlYWQgLXIgcmVmOyBkbyBydW5fcnVudGlwaSBhcHAgYmFja3VwICIkcmVmIjsgZG9uZTsgfQpkb2N0b3IoKSB7CiAgbG9jYWwgZmFpbHVyZXM9MAogIHByaW50ZiAnTWlzdGJvcm4gaG9zdCBkaWFnbm9zdGljc1xuJwogIGZvciBjb21tYW5kIGluIGRvY2tlciB0YWlsc2NhbGUgcmNsb25lIHVmdyBmYWlsMmJhbi1jbGllbnQ7IGRvCiAgICBpZiBjb21tYW5kIC12ICIkY29tbWFuZCIgPi9kZXYvbnVsbCAyPiYxOyB0aGVuIHByaW50ZiAnICBQQVNTICAlcyBpbnN0YWxsZWRcbicgIiRjb21tYW5kIjsgZWxzZSBwcmludGYgJyAgV0FSTiAgJXMgbWlzc2luZ1xuJyAiJGNvbW1hbmQiOyBmaQogIGRvbmUKICBpZiBbWyAtZCAiJFJVTlRJUElfUEFUSCIgXV07IHRoZW4gcHJpbnRmICcgIFBBU1MgIFJ1bnRpcGkgZGlyZWN0b3J5OiAlc1xuJyAiJFJVTlRJUElfUEFUSCI7IGVsc2UgcHJpbnRmICcgIEZBSUwgIFJ1bnRpcGkgZGlyZWN0b3J5IG1pc3Npbmdcbic7IGZhaWx1cmVzPTE7IGZpCiAgaWYgZG9ja2VyIGluZm8gPi9kZXYvbnVsbCAyPiYxOyB0aGVuIHByaW50ZiAnICBQQVNTICBEb2NrZXIgZGFlbW9uIHJlYWNoYWJsZVxuJzsgZWxzZSBwcmludGYgJyAgRkFJTCAgRG9ja2VyIGRhZW1vbiB1bnJlYWNoYWJsZVxuJzsgZmFpbHVyZXM9MTsgZmkKICBpZiBzeXN0ZW1jdGwgaXMtYWN0aXZlIC0tcXVpZXQgZmFpbDJiYW47IHRoZW4gcHJpbnRmICcgIFBBU1MgIGZhaWwyYmFuIGFjdGl2ZVxuJzsgZWxzZSBwcmludGYgJyAgV0FSTiAgZmFpbDJiYW4gaW5hY3RpdmVcbic7IGZpCiAgaWYgdWZ3IHN0YXR1cyAyPi9kZXYvbnVsbCB8IGdyZXAgLXEgJ1N0YXR1czogYWN0aXZlJzsgdGhlbiBwcmludGYgJyAgUEFTUyAgVUZXIGFjdGl2ZVxuJzsgZWxzZSBwcmludGYgJyAgV0FSTiAgVUZXIGluYWN0aXZlXG4nOyBmaQogIHJldHVybiAiJGZhaWx1cmVzIgp9CnN0YXR1cygpIHsKICBkb2N0b3IgfHwgdHJ1ZQogIHByaW50ZiAnXG5TZXJ2aWNlIHN0YXR1c1xuJwogIGxvY2FsIHNlcnZpY2UKICBmb3Igc2VydmljZSBpbiBkb2NrZXIgdGFpbHNjYWxlZCBmYWlsMmJhbjsgZG8KICAgIGlmICEgY29tbWFuZCAtdiBzeXN0ZW1jdGwgPi9kZXYvbnVsbCAyPiYxOyB0aGVuCiAgICAgIHByaW50ZiAnICBJTkZPICBzeXN0ZW1kIHVuYXZhaWxhYmxlOyBjYW5ub3QgaW5zcGVjdCAlc1xuJyAiJHNlcnZpY2UiCiAgICAgIGJyZWFrCiAgICBlbGlmIHN5c3RlbWN0bCBpcy1hY3RpdmUgLS1xdWlldCAiJHNlcnZpY2UiOyB0aGVuCiAgICAgIHByaW50ZiAnICBQQVNTICAlcyBhY3RpdmVcbicgIiRzZXJ2aWNlIgogICAgZWxzZQogICAgICBwcmludGYgJyAgV0FSTiAgJXMgaW5hY3RpdmVcbicgIiRzZXJ2aWNlIgogICAgZmkKICBkb25lCiAgaWYgY29tbWFuZCAtdiB0YWlsc2NhbGUgPi9kZXYvbnVsbCAyPiYxOyB0aGVuCiAgICBwcmludGYgJ1xuVGFpbHNjYWxlXG4nCiAgICB0YWlsc2NhbGUgc3RhdHVzIHx8IHRydWUKICBmaQp9CmZpeF9zZXJ2aWNlcygpIHsKICBsb2NhbCBzZXJ2aWNlIGNoYW5nZWQ9MAogIFtbICIkRVVJRCIgPT0gMCBdXSB8fCBkaWUgInJ1biAnbWlzdGJvcm4gZml4JyB3aXRoIHN1ZG8iCiAgY29tbWFuZCAtdiBzeXN0ZW1jdGwgPi9kZXYvbnVsbCAyPiYxIHx8IGRpZSAic3lzdGVtZCBpcyByZXF1aXJlZCB0byByZXBhaXIgc2VydmljZXMiCiAgZm9yIHNlcnZpY2UgaW4gZG9ja2VyIHRhaWxzY2FsZWQ7IGRvCiAgICBpZiAhIGNvbW1hbmQgLXYgIiRzZXJ2aWNlIiA+L2Rldi9udWxsIDI+JjE7IHRoZW4KICAgICAgcHJpbnRmICcgIFNLSVAgICVzIGlzIG5vdCBpbnN0YWxsZWRcbicgIiRzZXJ2aWNlIgogICAgICBjb250aW51ZQogICAgZmkKICAgIGlmIHN5c3RlbWN0bCBpcy1hY3RpdmUgLS1xdWlldCAiJHNlcnZpY2UiICYmIHN5c3RlbWN0bCBpcy1lbmFibGVkIC0tcXVpZXQgIiRzZXJ2aWNlIjsgdGhlbgogICAgICBwcmludGYgJyAgUEFTUyAgJXMgaXMgYWxyZWFkeSBlbmFibGVkIGFuZCBhY3RpdmVcbicgIiRzZXJ2aWNlIgogICAgZWxzZQogICAgICBwcmludGYgJyAgRklYICAgZW5hYmxpbmcgYW5kIHN0YXJ0aW5nICVzXG4nICIkc2VydmljZSIKICAgICAgc3lzdGVtY3RsIGVuYWJsZSAtLW5vdyAiJHNlcnZpY2UiCiAgICAgIGNoYW5nZWQ9MQogICAgZmkKICBkb25lCiAgaWYgW1sgIiRjaGFuZ2VkIiA9PSAwIF1dOyB0aGVuIHByaW50ZiAnQ29yZSBzZXJ2aWNlcyBhcmUgYWxyZWFkeSBoZWFsdGh5LlxuJzsgZmkKICBwcmludGYgJ1VGVyBhbmQgU1NIIHNldHRpbmdzIGFyZSBsZWZ0IHVuY2hhbmdlZDsgcmV2aWV3IHRoZW0gd2l0aCBtaXN0Ym9ybiBzZWN1cml0eS1zdGF0dXMuXG4nCn0KdXBkYXRlX3J1bnRpcGkoKSB7CiAgcHJpbnRmICdVcGRhdGluZyBSdW50aXBpIGNvcmUgKHdpdGggYXBwIHNuYXBzaG90cykuLi5cbicKICB0YXNrX2V2ZW50IGNvcmUgdXBkYXRlIHN0YXJ0ZWQKICBzbmFwc2hvdF9hcHBzIDwgPChhcHBfcmVmcykKICBydW5fcnVudGlwaSB1cGRhdGUgbGF0ZXN0CiAgdGFza19ldmVudCBjb3JlIHVwZGF0ZSBjb21wbGV0ZWQKICBwcmludGYgJ1xuVXBkYXRpbmcgYXBwIHN0b3Jlcy4uLlxuJwogIHRhc2tfZXZlbnQgYXBwc3RvcmVzIHVwZGF0ZSBzdGFydGVkCiAgcnVuX3J1bnRpcGkgYXBwc3RvcmUgdXBkYXRlCiAgdGFza19ldmVudCBhcHBzdG9yZXMgdXBkYXRlIGNvbXBsZXRlZAogIHByaW50ZiAnXG5VcGRhdGluZyBhcHBzICh3aXRoIHNuYXBzaG90cykuLi5cbicKICB0YXNrX2V2ZW50IGFwcHMgdXBkYXRlIHN0YXJ0ZWQKICB1cGRhdGVfYXBwcwogIHRhc2tfZXZlbnQgYXBwcyB1cGRhdGUgY29tcGxldGVkCiAgcHJpbnRmICdcbk1pc3Rib3JuIHVwZGF0ZXMgY29tcGxldGUuXG4nCn0KdXBkYXRlX2Jvb3RzdHJhcCgpIHsKICBbWyAiJEVVSUQiID09IDAgXV0gfHwgZGllICJydW4gJ21pc3Rib3JuIHVwZ3JhZGUnIHdpdGggc3VkbyIKICBsb2NhbCBsYXRlc3QgY3VycmVudCB0ZW1wX2RpciBpbnN0YWxsZXJfdXJsCiAgdGFza19ldmVudCBib290c3RyYXAgcmVsZWFzZSBzdGFydGVkCiAgcHJpbnRmICdDaGVja2luZyB0aGUgbGF0ZXN0IHN0YWJsZSBNaXN0Ym9ybiBCb290c3RyYXAgcmVsZWFzZS4uLlxuJwogIGxhdGVzdD0iJChjdXJsIC1mc1NMIGh0dHBzOi8vYXBpLmdpdGh1Yi5jb20vcmVwb3MvRWNrUGhpL21pc3Rib3JuLWJvb3RzdHJhcC9yZWxlYXNlcy9sYXRlc3QgfCBzZWQgLW5FICdzL15bWzpzcGFjZTpdXSoidGFnX25hbWUiOltbOnNwYWNlOl1dKiIodlswLTldK1wuWzAtOV0rXC5bMC05XSspIi4qL1wxL3AnIHwgaGVhZCAtbjEpIiB8fCBkaWUgImNvdWxkIG5vdCBjaGVjayBHaXRIdWIgcmVsZWFzZXMiCiAgW1sgIiRsYXRlc3QiID1+IF52WzAtOV0rXC5bMC05XStcLlswLTldKyQgXV0gfHwgZGllICJHaXRIdWIgcmV0dXJuZWQgbm8gc3RhYmxlIGJvb3RzdHJhcCByZWxlYXNlIgogIHRhc2tfZXZlbnQgYm9vdHN0cmFwIHJlbGVhc2UgY29tcGxldGVkCiAgY3VycmVudD0iJHtNSVNUQk9STl9CT09UU1RSQVBfVkVSU0lPTjotdjAuMC4wfSIKICBjdXJyZW50PSIke2N1cnJlbnQjdn0iCiAgbGF0ZXN0PSIke2xhdGVzdCN2fSIKICBpZiBbWyAiJChwcmludGYgJyVzXG4lc1xuJyAiJGN1cnJlbnQiICIkbGF0ZXN0IiB8IHNvcnQgLVYgfCB0YWlsIC1uMSkiID09ICIkY3VycmVudCIgXV07IHRoZW4KICAgIHByaW50ZiAnTWlzdGJvcm4gQm9vdHN0cmFwICVzIGlzIGFscmVhZHkgY3VycmVudC5cbicgIiRjdXJyZW50IgogICAgdGFza19ldmVudCBib290c3RyYXAgaW5zdGFsbGVyIHNraXBwZWQKICAgIHJldHVybiAwCiAgZmkKICBpbnN0YWxsZXJfdXJsPSJodHRwczovL3Jhdy5naXRodWJ1c2VyY29udGVudC5jb20vRWNrUGhpL21pc3Rib3JuLWJvb3RzdHJhcC92JHtsYXRlc3R9L2luc3RhbGwuc2giCiAgdGVtcF9kaXI9IiQobWt0ZW1wIC1kKSIKICB0cmFwICdybSAtcmYgIiR0ZW1wX2RpciInIFJFVFVSTgogIHRhc2tfZXZlbnQgYm9vdHN0cmFwIGluc3RhbGxlciBzdGFydGVkCiAgcHJpbnRmICdVcGRhdGluZyBNaXN0Ym9ybiBCb290c3RyYXAgJXMg4oaSICVzLi4uXG4nICIkY3VycmVudCIgIiRsYXRlc3QiCiAgY3VybCAtZnNTTCAiJGluc3RhbGxlcl91cmwiIC1vICIkdGVtcF9kaXIvaW5zdGFsbC5zaCIgfHwgZGllICJjb3VsZCBub3QgZG93bmxvYWQgYm9vdHN0cmFwIGluc3RhbGxlciIKICBNSVNUQk9STl9WRVJTSU9OPSJ2JHtsYXRlc3R9IiBNSVNUQk9STl9UVUk9MCBiYXNoICIkdGVtcF9kaXIvaW5zdGFsbC5zaCIgc2VydmVyIC0teWVzCiAgdGFza19ldmVudCBib290c3RyYXAgaW5zdGFsbGVyIGNvbXBsZXRlZAogIHByaW50ZiAnTWlzdGJvcm4gQm9vdHN0cmFwIHVwZGF0ZWQgdG8gJXMuXG4nICIkbGF0ZXN0Igp9CnVwZGF0ZV9hcHBzKCkgewogIGxvY2FsIGJhY2t1cD0xIHJlZnM9KCkgcmVmCiAgW1sgIiR7MTotfSIgPT0gLS1uby1iYWNrdXAgXV0gJiYgeyBiYWNrdXA9MDsgc2hpZnQ7IH0KICBpZiBbWyAkIyAtZ3QgMCBdXTsgdGhlbiByZWZzPSgiJEAiKTsgZWxzZSBtYXBmaWxlIC10IHJlZnMgPCA8KGFwcF9yZWZzKTsgZmkKICBmb3IgcmVmIGluICIke3JlZnNbQF19IjsgZG8gW1sgIiRiYWNrdXAiID09IDEgXV0gJiYgcnVuX3J1bnRpcGkgYXBwIGJhY2t1cCAiJHJlZiI7IHJ1bl9ydW50aXBpIGFwcCB1cGRhdGUgIiRyZWYiOyBkb25lCn0KdXNhZ2UoKSB7CiAgY2F0IDw8J0VPRicKVXNhZ2U6IG1pc3Rib3JuIENPTU1BTkQgW0FSR1NdCiAgc3RhdHVzICAgICAgICAgICAgICAgICAgICAgICBzaG93IGluc3RhbGxhdGlvbiwgc2VydmljZSBhbmQgVGFpbHNjYWxlIHN0YXR1cwogIGRvY3RvciAgICAgICAgICAgICAgICAgICAgICAgYXVkaXQgRG9ja2VyLCBSdW50aXBpLCBUYWlsc2NhbGUsIHJjbG9uZSBhbmQgc2VjdXJpdHkKICBmaXggICAgICAgICAgICAgICAgICAgICAgICAgIGVuYWJsZSBhbmQgc3RhcnQgaW5zdGFsbGVkIERvY2tlci9UYWlsc2NhbGUgc2VydmljZXMKICBzZWN1cml0eS1zdGF0dXMgICAgICAgICAgICAgIHNob3cgU1NILCBVRlcsIGZhaWwyYmFuIGFuZCBUYWlsc2NhbGUgc3RhdHVzCiAgdGFpbHNjYWxlLXN0YXR1cyAgICAgICAgICAgICBzaG93IFRhaWxzY2FsZSBzdGF0dXMKICByY2xvbmUtY29uZmlnICAgICAgICAgICAgICAgIG9wZW4gcmNsb25lJ3MgY29uZmlndXJhdGlvbiBVSQogIHVwZGF0ZS1hcHBzIFstLW5vLWJhY2t1cF0gW0FQUDpTVE9SRSAuLi5dCiAgdXBkYXRlLWNvcmUgWy0tbm8tYmFja3VwXSBbVkVSU0lPTl0KICB1cGRhdGUtYXBwc3RvcmVzCiAgdXBncmFkZSAgICAgICAgICAgICAgICAgICAgICB1cGdyYWRlIE1pc3Rib3JuIEJvb3RzdHJhcCB0byB0aGUgbGF0ZXN0IHN0YWJsZSByZWxlYXNlCiAgdXBkYXRlICAgICAgICAgICAgICAgICAgICAgICBhbGlhcyBmb3IgdXBncmFkZQogIHVwZGF0ZS1ydW50aXBpICAgICAgICAgICAgICAgdXBkYXRlIFJ1bnRpcGkgY29yZSwgYXBwIHN0b3JlcyBhbmQgYXBwcyAod2l0aCBiYWNrdXBzKQpFT0YKfQpjYXNlICIkezE6LX0iIGluCiAgaGVscHwtaHwtLWhlbHB8JycpIHVzYWdlIDs7CiAgc3RhdHVzKSBzdGF0dXMgOzsKICBkb2N0b3IpIGRvY3RvciA7OwogIGZpeCkgZml4X3NlcnZpY2VzIDs7CiAgc2VjdXJpdHktc3RhdHVzKSBzc2hkIC1UIDI+L2Rldi9udWxsIHwgZ3JlcCAtRSAncGFzc3dvcmRhdXRoZW50aWNhdGlvbnxwZXJtaXRyb290bG9naW58XnBvcnQnOyB1Zncgc3RhdHVzIHZlcmJvc2U7IGZhaWwyYmFuLWNsaWVudCBzdGF0dXMgc3NoZCB8fCB0cnVlOyB0YWlsc2NhbGUgc3RhdHVzIHx8IHRydWUgOzsKICB0YWlsc2NhbGUtc3RhdHVzKSB0YWlsc2NhbGUgc3RhdHVzIDs7CiAgcmNsb25lLWNvbmZpZykgcmNsb25lIGNvbmZpZyA7OwogIHVwZGF0ZS1hcHBzKSBzaGlmdDsgdXBkYXRlX2FwcHMgIiRAIiA7OwogIHVwZGF0ZS1jb3JlKSBzaGlmdDsgYmFja3VwPTE7IFtbICIkezE6LX0iID09IC0tbm8tYmFja3VwIF1dICYmIHsgYmFja3VwPTA7IHNoaWZ0OyB9OyBbWyAiJGJhY2t1cCIgPT0gMSBdXSAmJiBzbmFwc2hvdF9hcHBzIDwgPChhcHBfcmVmcyk7IHJ1bl9ydW50aXBpIHVwZGF0ZSAiJHsxOi1sYXRlc3R9IiA7OwogIHVwZGF0ZS1hcHBzdG9yZXMpIHJ1bl9ydW50aXBpIGFwcHN0b3JlIHVwZGF0ZSA7OwogIHVwZ3JhZGV8dXBkYXRlKSB1cGRhdGVfYm9vdHN0cmFwIDs7CiAgdXBkYXRlLXJ1bnRpcGkpIHVwZGF0ZV9ydW50aXBpIDs7CiAgKikgZGllICJ1bmtub3duIGNvbW1hbmQ6ICQxIiA7Owplc2FjCg=='
export MISTBORN_UPDATE_PLAN_B64='dmVyc2lvbiA9IDEKY29sbGVjdGlvbiA9ICJ1cGRhdGUiCgpbW3N0YWdlc11dCmlkID0gImJvb3RzdHJhcCIKdGl0bGUgPSAiTWlzdGJvcm4gQm9vdHN0cmFwIgpoZWxwID0gIkNoZWNrcyB0aGUgbGF0ZXN0IHN0YWJsZSByZWxlYXNlIGFuZCByZXJ1bnMgaXRzIGluc3RhbGxlciB0byByZWZyZXNoIHRoZSBNaXN0Ym9ybiB0b29sIHdoaWxlIHJlc3VtaW5nIGNvbXBsZXRlZCBzZXR1cCBzdGFnZXMuIgpbW3N0YWdlcy50YXNrc11dCmlkID0gInJlbGVhc2UiCnRpdGxlID0gIkNoZWNrIGxhdGVzdCBzdGFibGUgYm9vdHN0cmFwIHJlbGVhc2UiCmFjdGlvbiA9ICJHaXRIdWIgcmVsZWFzZXMvbGF0ZXN0Igp3ZWlnaHQgPSAxCltbc3RhZ2VzLnRhc2tzXV0KaWQgPSAiaW5zdGFsbGVyIgp0aXRsZSA9ICJSZWZyZXNoIE1pc3Rib3JuIHJ1bm5lciBhbmQgY29tbWFuZCIKYWN0aW9uID0gInJ1biB0YWdnZWQgc2VydmVyIGluc3RhbGxlciAocmVzdW1lIGNvbXBsZXRlZCBzdGFnZXMpIgp3ZWlnaHQgPSAzCg=='
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
  mistborn_task_start apt-index
  mistborn_run apt-get update
  mistborn_task_complete apt-index
  mistborn_task_start base-packages
  mistborn_apt_install ca-certificates curl git
  mistborn_task_complete base-packages
  ui_success "$module_common_description"
}
# shellcheck shell=bash

module_zsh_description="Zsh, Oh My Zsh, and Powerlevel10k"

module_zsh_apply() {
  local user home custom_dir zshrc
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }

  ui_step "$module_zsh_description for $user"
  mistborn_task_start packages
  mistborn_apt_install zsh git
  mistborn_task_complete packages
  custom_dir="$home/.oh-my-zsh"
  mistborn_task_start oh-my-zsh
  if [[ ! -d "$custom_dir/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/ohmyzsh/ohmyzsh.git "$custom_dir"
  else
    ui_info "Oh My Zsh already installed"
  fi
  mistborn_task_complete oh-my-zsh
  mistborn_task_start powerlevel10k
  if [[ ! -d "$custom_dir/custom/themes/powerlevel10k/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/romkatv/powerlevel10k.git \
      "$custom_dir/custom/themes/powerlevel10k"
  else
    ui_info "Powerlevel10k already installed"
  fi
  mistborn_task_complete powerlevel10k

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
  ui_success "$module_zsh_description"
}
main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run) MISTBORN_DRY_RUN=1 ;;
      --yes) MISTBORN_YES=1 ;;
      --user) shift; MISTBORN_USER="${1:?--user requires a value}" ;;
      --only) shift; MISTBORN_ONLY="${1:?--only requires a module name}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME] [--only MODULE]\n' "$0"; return ;;
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
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Mistborn shell setup"
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != common ]] || module_common_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != zsh ]] || module_zsh_apply
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Setup complete"
}
main "$@"
