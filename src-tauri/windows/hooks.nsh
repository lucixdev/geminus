; This file is part of GEMINUS.
;
; Copyright (C) 2026 lucix.dev <lucix.dev@proton.me>
;
; GEMINUS is free software: you can redistribute it and/or modify it under
; the terms of the GNU General Public License as published by the Free
; Software Foundation, either version 3 of the License, or (at your option)
; any later version.
;
; GEMINUS is distributed in the hope that it will be useful, but WITHOUT ANY
; WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
; FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
; details.
;
; You should have received a copy of the GNU General Public License along
; with GEMINUS. If not, see <https://www.gnu.org/licenses/>.
;
; SPDX-License-Identifier: GPL-3.0-or-later

; Explorer context menu entries for folders and drives (per-user, HKCU).

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Folders
  WriteRegStr HKCU "Software\Classes\Directory\shell\GeminusDiskA" "" "Imposta come Disco A in GEMINUS"
  WriteRegStr HKCU "Software\Classes\Directory\shell\GeminusDiskA" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  WriteRegStr HKCU "Software\Classes\Directory\shell\GeminusDiskA\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --disk-a "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\shell\GeminusDiskB" "" "Imposta come Disco B in GEMINUS"
  WriteRegStr HKCU "Software\Classes\Directory\shell\GeminusDiskB" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  WriteRegStr HKCU "Software\Classes\Directory\shell\GeminusDiskB\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --disk-b "%1"'

  ; Drives
  WriteRegStr HKCU "Software\Classes\Drive\shell\GeminusDiskA" "" "Imposta come Disco A in GEMINUS"
  WriteRegStr HKCU "Software\Classes\Drive\shell\GeminusDiskA" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  WriteRegStr HKCU "Software\Classes\Drive\shell\GeminusDiskA\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --disk-a "%1"'
  WriteRegStr HKCU "Software\Classes\Drive\shell\GeminusDiskB" "" "Imposta come Disco B in GEMINUS"
  WriteRegStr HKCU "Software\Classes\Drive\shell\GeminusDiskB" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  WriteRegStr HKCU "Software\Classes\Drive\shell\GeminusDiskB\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --disk-b "%1"'

  ; Tell the shell that associations changed
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\shell\GeminusDiskA"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\GeminusDiskB"
  DeleteRegKey HKCU "Software\Classes\Drive\shell\GeminusDiskA"
  DeleteRegKey HKCU "Software\Classes\Drive\shell\GeminusDiskB"

  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
!macroend
