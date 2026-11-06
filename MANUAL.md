<!--
  This file is part of GEMINUS.

  Copyright (C) 2026 lucix.dev <lucix.dev@proton.me>

  GEMINUS is free software: you can redistribute it and/or modify it under
  the terms of the GNU General Public License as published by the Free
  Software Foundation, either version 3 of the License, or (at your option)
  any later version.

  GEMINUS is distributed in the hope that it will be useful, but WITHOUT ANY
  WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
  FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
  details.

  You should have received a copy of the GNU General Public License along
  with GEMINUS. If not, see <https://www.gnu.org/licenses/>.

  SPDX-License-Identifier: GPL-3.0-or-later
-->
# GEMINUS — Manual / Manuale

**English** · **[Italiano](#manuale---italiano)**

---

## English

### Table of contents

1. [About GEMINUS](#about-geminus)
2. [How to use](#how-to-use)
   - [Schedule](#schedule)
3. [Slow or faulty disks](#slow-or-faulty-disks)
4. [Formatting a disk](#formatting-a-disk)
5. [Frequently asked questions](#frequently-asked-questions)

---

### About GEMINUS

#### What GEMINUS is

GEMINUS is a desktop application that **compares two disks or two folders**. It shows side by side which files are identical, which have been modified, which exist on only one side.

#### Where it runs

On Linux and on Windows, with the same features. On Windows it needs version 10 22H2 or later, 64 bit.

The few things that differ between the two systems are marked throughout this manual with **On Linux** and **On Windows**. Where there's no marker, it holds for both.

#### What it's for

- Verify that a backup is complete and identical to the source.
- Find modified files between two copies of a folder.
- Discover whether an old disk holds files missing from a new one.
- Copy or move files and folders between sides by dragging them.
- Delete from the backup what no longer exists in the source.

#### How it works

When you click **▶ Compare**, GEMINUS asks how you want to compare the disks. There are three methods:

- **Quick**. GEMINUS looks at file names, sizes and dates. Fast, but if two files have the same size and the same date it treats them as identical even if they're different inside.
- **Deep**. GEMINUS also reads the contents of files that look the same (same name, same size, same date), to be sure they really are identical inside. Files that differ it spots right away without reading them. Slow, but reliable.
- **Full**. Like Deep, plus it checks the physical health of the disks before starting. To do that it asks for permission: *on Linux* the administrator password, *on Windows* your consent in the system's own window.

#### When to use which method

For everyday use (seeing what changed after a copy): **Quick**.

For checking an important backup (years of photos, documents that really matter to you): **Deep**.

When you have doubts about the disk (old USB drive, suspicion that something isn't right): **Full**.

#### Technology

GEMINUS is written in Rust on the Tauri framework. It is free software distributed under GPL v3. Your data always stays on your computer: nothing is sent over the network.

---

### How to use

#### Look and feel

GEMINUS has a Windows 11 look (Segoe UI font). The theme follows the system, light or dark, and its accent color. The theme button stays there to force one or the other.

#### Basic compare

1. Click on the **Drive A** block in the top left. The folder picker opens, in the style of File Explorer: pick a folder and press **Select this folder**. GEMINUS remembers the last paths of A and B and offers them again at the next start.
2. Same with the **Drive B** block on the right.
3. Click **▶ Compare** in the center. GEMINUS asks which method to use: **Quick**, **Deep** or **Full**. Choose, read the confirmation, press **Proceed**.

If you chose **Full**, GEMINUS asks for permission to check the disks' health — *on Linux* the administrator password, *on Windows* your consent in the system's own window. If you don't grant it, you can carry on with the Deep Comparison alone or go back to the method selection.

The health check relies on a free external program, installed once and for all. If it isn't there, GEMINUS tells you and explains how to install it the way your system does it; you can also carry on without, and then Full counts as Deep. Comparing and copying don't depend on that program: they work either way.

#### The folder picker

It works like File Explorer:

- **Back**, **Forward**, **Up** and **Refresh** buttons.
- **Address bar** made of clickable segments: click a segment to jump there, click the empty part to type a path.
- **Side panel** with Quick access, This PC and the drives. *On Linux* the root of the filesystem is there too; *on Windows* there is no single root, the drives (`C:`, `D:`, …) are under This PC.
- **Name** and **Type** columns. A click selects, a double click goes in. The **Folder** field shows what will be chosen.

This PC and the list of drives are not folders and cannot be confirmed: go into a drive and choose in there.

#### What you see after the compare

The two side-by-side trees show the contents of the two disks, row by row: where an item is missing on one side, an **(absent)** row takes its place, so the columns stay aligned at any depth. Rows are color-coded by status: **blue** = only in A, **green** = only in B, **orange** = modified, and a badge labels each one. The status bar at the bottom summarizes the counts.

Folders reflect the state of what's inside them: if even one file within a folder is different or missing on one side, the folder itself is marked Modified. Expand it to see what's changed.

After a **Quick** comparison a warning stays above the trees, and for good reason: that comparison did not read file contents, so there "identical" means "same size and same date".

#### Filters and search

The chips at the top filter by status: `All`, `Modified`, `Only in A`, `Only in B`, `Same in A and B`, `Unreadable`. Search filters by name.

The `Hidden` chip also shows the items your system considers hidden: *on Linux* those whose name starts with a dot, *on Windows* those carrying the hidden attribute. An item hidden on one side and plain on the other stays visible anyway, because that is a difference. Worth knowing: the same disk compared from Linux and from Windows can show a different number of hidden files — what changes is the notion of hidden, not the disk.

The numbers on the chips count only files. Folders appear colored in the tree but are not part of the totals.

#### Columns, sorting and icons

Both lists show **Name**, **Size**, **Date modified** and **Type**. One click on a header sorts by that column (Name, Size, Date, Type); click again to reverse. Each file has an icon for its type. Long names are cut: hover over one to see the full path in a tooltip.

#### Copy or move files

Drag a file or a folder from one column to the other. It works like Windows: with `Ctrl` held it **copies**, with `Shift` it **moves**; with no key it follows the mode (`Copy` or `Move`) chosen in the toolbar.

Dropping **always** puts the item in its mirror position on the other side: same folder, same place. It doesn't matter which row you drop it on — GEMINUS aligns the two sides, it doesn't rearrange the disk.

**Several items at once:** `Ctrl`+click adds a row to the selection or takes it out, `Shift`+click takes every row between the last one clicked and this one. You select in one column only. Dragging a selected row carries the whole group, each item to its own mirror position; if the group holds a folder, what is inside it travels with it. Only what you see is carried: changing filter, searching or closing a folder takes the rows that disappear out of the selection. `Esc` or a plain click on a file clears it; a plain click on a folder opens or closes it and leaves the selection alone, so you can pick rows from several folders.

If something with the same name is already on the other side, GEMINUS asks only when something could be lost. A missing file is copied; an identical file (✓) is copied over without a question, since nothing changes; a folder that is already there is not replaced: GEMINUS goes into it and completes its contents, with the same rules for the files inside.

The question comes only for files with the same name that are **different**, because overwriting them deletes the version that was there for good. For a file dragged alone: **Overwrite**, **Rename** or **Cancel**. For a group or a folder, before starting, one window lists them all, including the ones inside folders: **Overwrite all**, **Skip these** (they stay as they are, everything else goes), **One by one** (Overwrite, Rename or Cancel for each) or **Cancel** (nothing starts).

Write-protected files get replaced too. The one case where it can't is when the destination folder itself refuses to be written to, and then it tells you instead of pretending.

The tree updates as you go: every file copied, moved, or deleted changes status in the view right away. There's no need to run the compare again.

**Double click** on a file opens it with the system's default program.

Links (🔗) can't be dragged. And if you drag a folder that contains some, the links inside **are not copied**: GEMINUS skips them.

When an operation ends, the status bar at the bottom says how it went: copied, moved, skipped, cancelled, or partial copy with the number of files left behind. If something didn't go through in full — a folder copied halfway, links skipped, an operation stopped — a summary also opens, with the counts and the destination.

In `Move` mode, if any file of a folder was left behind, **the source folder is not removed**: that would be the one way to lose what didn't make it. You find it where it was and whole, including the files that already reached the other side: nothing is removed from the source, and the summary says so.

#### Delete files

Select one or more rows and press `Del`, or right click → **Recycle Bin**. The item goes to the system's Recycle Bin (*on Linux*, the Trash) and can be restored from there.

`Shift`+`Del` deletes **permanently**: no Recycle Bin, no undo. The same happens if the drive has no Recycle Bin (some USB drives and network shares): GEMINUS tells you, and deletes permanently only after you confirm. The confirmation names the item and says it **will be permanently deleted**. The button already primed is Cancel: to delete, you have to choose it yourself.

On a folder, deleting takes away **everything inside**, subfolders included, in one go. The confirmation names the folder, it doesn't list the contents: look at what's in there before you confirm.

It can also stop halfway — a protected file, a folder that will not be touched. GEMINUS says so, and the view is rebuilt on what is actually still on the disk: what you see afterwards is the real situation, not the one from before.

In normal use the net is the comparison itself: a file deleted from the backup by mistake still exists in the source, and one comparison plus one drag puts it back. What doesn't come back is what you delete permanently from the side where it was the only copy: before confirming, look at which column you're on.

On links (🔗), deleting removes the link and doesn't touch the file or folder it points to.

#### Context menus

Right click on a **file or folder**: Open, Copy / Move to the other drive, Rename, Copy path, Recycle Bin (or Delete).

Right click on a **Drive A / B block**: Open, Change folder, Compare, Check drive health, Copy path.

#### Keyboard shortcuts

| Keys | Action |
|---|---|
| `Ctrl`+`C` / `Ctrl`+`X` / `Ctrl`+`V` | Copy / move the selection to the other drive |
| `Del` | Recycle Bin |
| `Shift`+`Del` | Delete permanently |
| `F5` | Repeat the comparison |
| `F2` | Rename |
| `Ctrl`+`F` | Search |
| `Ctrl`+`A` | Select all |
| `Esc` | Clear the selection |

#### Favorites

The **★ Favorites** menu saves the current A↔B pair under a name, loads a saved pair back, or removes it.

#### Schedule

The **⏱ Schedule** button at the top opens the window of scheduled tasks: GEMINUS compares two folders by itself at the times you choose and, if you want, aligns them by copying.

The window lists the tasks, each with its **name**, its **disks** (A and B), **when** it runs, its **next run** and the **outcome of the last run**. Each task has four buttons: **Run now**, **Edit**, **Log** and **Delete**. The **Log** button opens the task's log, where every run is written down.

**New task** asks for:

- **Name** of the task.
- **Drive A** and **Drive B**, chosen with the usual folder picker.
- **Comparison**: **Quick** or **Deep**. **Full** cannot be scheduled, because it needs the administrator's permission.
- **Rules for differences**, one for each case:
  - *Only in A*: copy from A to B / do nothing.
  - *Only in B*: copy from B to A / do nothing.
  - *Different files*: the most recent wins / copy A→B (overwrites B) / copy B→A (overwrites A) / do nothing.
- **When**: every day at a time, every week on chosen days at a time, or every N hours (from 1 to 23).

What to know before you rely on it:

- **GEMINUS must be open.** Tasks start only while the program is running, even minimized to an icon. If a due time goes by while the app is closed, the task runs once when you open it.
- **A task never deletes anything.** It only copies; the *do nothing* rules leave the differences as they are.
- **It checks the disks first.** Before copying, GEMINUS verifies that the disks are the ones saved with the task (the name or identifier of the drive). If they don't match, the run is skipped and the log says so. This helps with USB drives that change letter when you plug them in.
- **A bad file doesn't stop the run.** A file that gives an error is skipped and noted in the log; the others carry on.
- **The window stays busy during a run.** While a scheduled task is running, you can't start other comparisons or copies from the window.

#### Taskbar progress

During comparisons and copies the progress also shows on the GEMINUS button in the Windows taskbar.

#### Windows Explorer integration (Windows only)

The installer adds two entries to the right click menu of any folder or drive in File Explorer: **Set as Drive A in GEMINUS** and **Set as Drive B in GEMINUS**. If GEMINUS is already open it reuses the window. From the command line:

```
geminus.exe --disk-a <path> --disk-b <path>
```

Dragging from File Explorer onto the GEMINUS window is **not** supported: use the menu entries or the picker.

#### When a file has trouble

If during a copy or move a file can't be read or written, GEMINUS stops and opens a window that says **where the item is**, **what failed** (reading from the source disk, writing to the destination disk, creating a folder, reading the contents of a folder) and **why**: permissions don't allow it, space has run out, the destination disk is read-only, the file is open in another program, the disk stopped responding, the disk reports a physical error. The system's raw text follows at the end, for whoever wants to read it.

Then it asks what to do: **Retry** (try the same file again, useful if the block was temporary), **Skip** (leave that file and move on with the rest), **Cancel** (stop everything now). Files already copied before the error stay where they are, and the one that failed leaves nothing behind on the destination side: what was there before is still there.

A folder that had a file skipped inside it is not shown as aligned: its status is decided by its contents, not by how the copy ended.

#### Extended Health Check

As soon as you pick a disk, the 🩺 icon appears on its block: that's the **Extended Health Check**, and you can launch it whenever you want — the quick check doesn't need to have flagged anything.

GEMINUS starts the disk's internal test, the one disks call SMART: the disk diagnoses itself, GEMINUS waits. The disk stays usable during the test, but slower, and you can cancel at any time.

How long it takes is declared by the disk, and it can be hours. GEMINUS reads that figure when it runs a **Full** comparison: if you've already run one on that disk, the window tells you how much is left and warns you if the test is overrunning its estimate; if you never have, it only tells you how long it's been working.

When the test is over the result is saved as a text file **in your downloads folder**, and GEMINUS tells you the exact path.

Permission is needed here too, but here the system asks for it directly — in Full it was GEMINUS asking you first, in its own window. If you refuse, the check doesn't start and nothing else happens. And if the external program isn't installed there's nothing to start: the window explains how to install it, and from there you go back whichever button you press.

#### Cancel an operation

During the scan and the comparison there's always a window with a `Cancel` button. In copies and moves it appears when there's something to wait for — a folder or a large file: for a small file the operation is over before stopping it would matter.

Cancelling is clean: the copy in progress is thrown away and the file that was already on the destination side stays exactly as it was, untouched. The ones already finished stay where they are, and the view shows the real situation — not the one expected before stopping.

---

### Slow or faulty disks

#### What bad sectors are

Every disk is physically divided into small zones called **sectors**. Over time, some zones stop working correctly (bad sectors). When GEMINUS tries to read a file located on a bad sector, the disk may respond slowly, return an error, or not respond at all.

#### Message "Waiting for disk..."

Appears while reading file contents when the disk hasn't responded for a few seconds. **It's not an error**: GEMINUS is waiting patiently. If the disk unblocks, the bar reverts to "Reading file contents..." and continues. What to do: nothing, let it work. What NOT to do: don't close the window, don't unplug the disk.

This doesn't happen in Quick mode: in Quick mode GEMINUS doesn't read file contents, so it never has to wait for the disk.

#### Counter "unreadable"

At the end of the compare, in the status: `(N Unreadable)`. These are the files GEMINUS could not read: the disk did not answer within the time limit, or returned an error, or permissions did not allow it. GEMINUS marked them unreadable and moved on. Filter the tree with the `Unreadable` chip to see them.

#### Counter "skipped — disk stuck"

Appears only in serious cases: `(N skipped — disk stuck)`. Means the disk got stuck so many times (8 failed files) that GEMINUS gave up and skipped the remaining files to avoid wasting time and resources.

#### What to do when you see these numbers

- **Few unreadable (1-3)**: probably isolated bad sectors. Make an urgent backup of the still-readable files to another disk.
- **Many unreadable or skipped**: the disk is seriously compromised. Salvage what you can, then go to the [Formatting a disk](#formatting-a-disk) section to reformat with sector check, or replace the disk if it's at end of life.
- **Disk with many sectors waiting to be remapped** (dozens or more): consider reformatting it with sector checking on, so the faulty ones get marked and the system stops using them. How to do it, on both systems, is in the [Formatting a disk](#formatting-a-disk) chapter.

---

### Formatting a disk

> ⚠ **Warning**: Formatting a disk wipes all data. Double-check which disk you are formatting before you proceed. Picking the wrong disk means losing everything.

#### First of all — save what can be recovered

**Before formatting, copy elsewhere everything still readable.** Formatting wipes everything, including from healthy sectors. Use GEMINUS or your system's file manager to copy data to another disk. Only when you're sure you have nothing left to save, proceed.

From here on the two paths differ: formatting is something the system does, not GEMINUS.

#### On Windows

File Explorer is all you need.

1. Open **File Explorer** and find the drive in the list on the left.
2. Right click the drive → **Format**.
3. Pick the file system: **NTFS** if the disk will stay on Windows, **exFAT** if you'll also use it on Linux or Mac (it handles files of any size), **FAT32** only for old devices that understand neither of the other two — a single file cannot exceed 4 GB, and Windows only offers it for small drives.
4. **Uncheck "Quick Format"**. This is the part that matters: the quick one doesn't look at the disk, the full one goes over every sector and marks the faulty ones, so the system stops using them. On a large disk it can take hours.
5. Press **Start** and wait for it to finish.

If instead you only want to check an ailing disk **without wiping anything**, open Command Prompt as administrator and run `chkdsk X: /r`, with `X` replaced by the drive letter: it looks for damaged sectors and recovers what it can read. This too can take hours.

#### On Linux

Here you go through the terminal, and the commands deserve attention: identify the disk before touching it.

##### Step 1 — identify the disk

Open a terminal and run:

```bash
lsblk -f
```

Recognize your USB by size and label. Example: a 4 TB USB might be `/dev/sdb` with partition `/dev/sdb1`. **Never use `/dev/sda`** unless you're absolutely sure: it's usually the system disk.

##### Step 2 — unmount

```bash
sudo umount /dev/sdX1
```

Replace `X` with the actual letter (e.g. `sdb1`). If the disk wasn't mounted, ignore the error.

##### Step 3 — format (pick a filesystem)

**ext4 — recommended for Linux, with sector check:**

```bash
sudo mkfs.ext4 -c -L "MyDisk" /dev/sdX1
```

The `-c` option runs a single-pass read check during formatting. For a deeper check (reads *and* writes, much slower, hours on large disks):

```bash
sudo mkfs.ext4 -cc -L "MyDisk" /dev/sdX1
```

**ext4 with badblocks pre-scan (most rigorous):**

```bash
sudo badblocks -wsv -o badblocks.txt /dev/sdX1
sudo mkfs.ext4 -l badblocks.txt -L "MyDisk" /dev/sdX1
```

`badblocks -wsv` overwrites every sector with 4 patterns and verifies. Destroys all data. Saves the bad-sector list that `mkfs.ext4 -l` excludes from the filesystem.

**FAT32 — universal compatibility (Windows, Mac, Linux, consoles). Limit: single files max 4 GB.**

```bash
sudo mkfs.vfat -F 32 -n MYDISK /dev/sdX1
```

FAT label is max 11 uppercase characters.

**NTFS — Windows, supports large files. Requires the `ntfs-3g` package.**

```bash
sudo mkfs.ntfs -Q -L "MyDisk" /dev/sdX1
```

`-Q` = quick format. Without `-Q` the check is deeper but takes hours.

**exFAT — Windows/Mac compatibility, files >4 GB. Requires `exfatprogs`.**

```bash
sudo mkfs.exfat -L "MyDisk" /dev/sdX1
```

##### Step 4 — remount

Physically unplug the USB and plug it back in: the file manager will auto-mount it. Alternatively:

```bash
sudo mkdir -p /mnt/mydisk
sudo mount /dev/sdX1 /mnt/mydisk
```

##### Step 5 — verify

```bash
df -h | grep sdX
```

Check that the total space matches expectations and that the filesystem is the right one.

---

### Frequently asked questions

**Does GEMINUS modify my files?**
Not on its own. The compare is read-only: it looks and touches nothing. Files change only when you ask, and there are two ways: dragging something from one column to the other, or deleting with the right button. In the first case, if the other side has a file with the same name but different, GEMINUS always asks what to do before overwriting it; in the second it asks for confirmation, and the item goes to the Recycle Bin unless you chose permanent deletion.

**Is my data sent over the internet?**
No. GEMINUS works completely offline. No telemetry, no cloud, no network connection. Everything stays on your computer.

**How long does a compare take?**
Quick is fast, even on large disks. Deep can take hours on large disks, because it reads the contents of the files. Full is like Deep, plus the health check. If the disk has problems, times stretch out.

**Can I use GEMINUS on Windows or Mac?**
On Linux and on Windows, yes — on Windows it needs version 10 22H2 or later, 64 bit. On Mac, no.

**What exactly does "Extended Health Check" do?**
It starts the disk's internal test: the disk diagnoses itself, GEMINUS waits and reports the outcome. How long it takes is declared by the disk, and it can be hours. The result is saved as a text file in your downloads folder.

**Can I cancel during a long compare?**
Yes. The comparison window always has a **Cancel** button. The app stops cleanly, without leaving files halfway.

**Why are some folders "excluded"?**
GEMINUS skips the folders that keep changing by themselves, where comparing would say nothing useful and would cost time: `.git`, `node_modules`, `target`, `.cargo`, `.rustup`, `.venv`, `.cache`, `__pycache__`, trash. *On Windows* it also skips the folders the system keeps for itself (`$RECYCLE.BIN`, `System Volume Information`, `Config.Msi`), the ones sitting in the root of every drive: they aren't yours, and comparing them says nothing about your backup. The name is what counts: a folder of yours named like that would be skipped all the same. This is about comparing, not about copying: drag a folder that contains one of these and it is copied whole — copying a folder has to give you the folder, not a version of it with pieces missing.

**How reliable is the Deep comparison?**
Very. GEMINUS reads the contents of both files from beginning to end: if they differ by even one byte, it sees it. The case where two different files are declared identical is so improbable that it won't happen in any human lifetime.

**Where do deleted files go?**
To the system's Recycle Bin (*on Linux*, the Trash), where you can restore them. Only `Shift`+`Del`, or a drive without a Recycle Bin and after your confirmation, deletes for good.

**Can I drag files from File Explorer into GEMINUS?**
No. Use the right click entries "Set as Drive A / B in GEMINUS" or the folder picker.

**Can I schedule with the program closed?**
No, for now the app has to be open (minimized is fine). If a due time passes while it is closed, the task runs once when you open it. See [Schedule](#schedule).

---
---

## Manuale — italiano

**[English](#english)** · **Italiano**

### Indice

1. [Cosa È GEMINUS](#cosa-è-geminus)
2. [Come si usa](#come-si-usa)
   - [Pianificazione](#pianificazione)
3. [Dischi lenti o difettosi](#dischi-lenti-o-difettosi)
4. [Formattare un disco](#formattare-un-disco)
5. [Domande frequenti](#domande-frequenti)

---

### Cosa È GEMINUS

#### Cosa È

GEMINUS è un'applicazione desktop che **confronta due dischi o due cartelle**. Mostra fianco a fianco quali file sono uguali, quali sono stati modificati, quali esistono solo da una parte.

#### Dove gira

Su Linux e su Windows, con le stesse funzioni. Su Windows serve la versione 10 22H2 o successiva, a 64 bit.

Le poche cose che cambiano da un sistema all'altro sono segnalate lungo il manuale con **Su Linux** e **Su Windows**. Dove non c'è nessuna indicazione, vale per tutti e due.

#### A cosa serve

- Verificare che un backup sia completo e identico all'originale.
- Trovare i file modificati fra due copie di una cartella.
- Capire se un disco vecchio contiene file che mancano in quello nuovo.
- Copiare o spostare file e cartelle da una parte all'altra trascinandoli.
- Cancellare dal backup quello che nell'originale non c'è più.

#### Come lavora

Quando clicchi **▶ Confronta**, GEMINUS ti chiede in che modo vuoi confrontare i dischi. Ci sono tre metodi:

- **Veloce**. GEMINUS guarda i nomi dei file, le dimensioni e le date. Rapido, ma se due file hanno stessa dimensione e stessa data li considera uguali anche se dentro sono diversi.
- **Approfondito**. GEMINUS legge anche il contenuto dei file che sembrano uguali (stesso nome, stessa dimensione, stessa data), per essere sicuro che siano davvero identici dentro. I file diversi li riconosce subito senza leggerli. Lento, ma sicuro.
- **Completo**. Come Approfondito, e in più controlla lo stato di salute fisica dei dischi prima di partire. Per farlo chiede un permesso: *su Linux* la password di amministratore, *su Windows* il consenso nella finestra del sistema.

#### Quando usare quale metodo

Per uso quotidiano (vedere cos'è cambiato dopo una copia): **Veloce**.

Per verificare un backup importante (le foto di anni, documenti che ti servono davvero): **Approfondito**.

Quando hai dubbi sul disco (USB vecchia, sospetto che qualcosa non vada): **Completo**.

#### Tecnologia

GEMINUS è scritto in Rust col framework Tauri. È software libero distribuito sotto licenza GPL v3. I dati restano sempre sul tuo computer: niente viene inviato in rete.

---

### Come si usa

#### Aspetto

GEMINUS ha un aspetto in stile Windows 11 (font Segoe UI). Il tema segue il sistema, chiaro o scuro, e il suo colore d'accento. Il pulsante tema resta per forzare l'uno o l'altro.

#### Confronto base

1. Clicca sul blocco **Disco A** in alto a sinistra. Si apre il selettore cartella, in stile Esplora file: scegli una cartella e premi **Scegli questa cartella**. GEMINUS ricorda gli ultimi percorsi di A e B e te li ripropone al prossimo avvio.
2. Stessa cosa col blocco **Disco B** a destra.
3. Clicca **▶ Confronta** al centro. GEMINUS ti chiede quale metodo usare: **Veloce**, **Approfondito** o **Completo**. Scegli, leggi la conferma, premi **Procedi**.

Se hai scelto **Completo**, GEMINUS chiede il permesso di controllare la salute dei dischi — *su Linux* la password di amministratore, *su Windows* il consenso nella finestra del sistema. Se non lo dai, puoi continuare col solo Confronto Approfondito o tornare alla scelta del metodo.

Il controllo della salute si appoggia a un programma esterno gratuito, che si installa una volta sola. Se non lo trova, GEMINUS te lo dice e ti spiega come installarlo nel modo del tuo sistema; puoi anche continuare senza, e allora il Completo vale come Approfondito. Confronto e copia non dipendono da quel programma: funzionano comunque.

#### Il selettore cartella

Funziona come Esplora file:

- Pulsanti **Indietro**, **Avanti**, **Su** e **Aggiorna**.
- **Barra degli indirizzi** a segmenti cliccabili: clicca un segmento per saltarci, clicca la parte vuota per digitare un percorso.
- **Pannello laterale** con Accesso rapido, Questo PC e le unità. *Su Linux* c'è anche la radice del sistema; *su Windows* una radice unica non esiste, le unità (`C:`, `D:`, …) stanno sotto Questo PC.
- Colonne **Nome** e **Tipo**. Un clic seleziona, un doppio clic entra. Il campo **Cartella** mostra cosa verrà scelto.

Questo PC e l'elenco delle unità non sono cartelle e non si possono confermare: entra in un'unità e scegli lì dentro.

#### Cosa vedi dopo il confronto

I due alberi affiancati mostrano il contenuto dei due dischi, riga per riga: dove un elemento manca da una parte, al suo posto compare la riga **(assente)**, così le due colonne restano allineate a qualsiasi profondità. Le righe sono colorate per stato: **azzurro** = solo in A, **verde** = solo in B, **arancione** = modificato, e un badge le etichetta una per una. La barra in basso riassume i conteggi.

Le cartelle riflettono lo stato di ciò che contengono: se anche un solo file dentro una cartella è diverso o manca da una parte, la cartella stessa è marcata come Modificata. Espandila per vedere cosa cambia.

Dopo un confronto **Veloce** resta un avviso sopra gli alberi, e ha un motivo: quel confronto non ha letto il contenuto dei file, quindi lì "uguale" significa "stessa dimensione e stessa data".

#### Filtri e ricerca

I chip in alto filtrano per stato: `Tutti`, `Modificati`, `Solo in A`, `Solo in B`, `Uguali in A e B`, `Illeggibili`. La ricerca filtra per nome.

Il chip `Nascosti` mostra anche gli elementi che il tuo sistema considera nascosti: *su Linux* quelli col nome che inizia per punto, *su Windows* quelli che portano l'attributo nascosto. Un elemento nascosto da una parte e normale dall'altra resta visibile comunque, perché quella è una differenza. Conseguenza da sapere: lo stesso disco confrontato da Linux e da Windows può mostrare un numero diverso di file nascosti — è la nozione di nascosto che cambia, non il disco.

I numeri sui chip contano solo i file. Le cartelle si vedono colorate nell'albero ma non rientrano nei totali.

#### Colonne, ordinamento e icone

Le due liste mostrano **Nome**, **Dimensione**, **Data modifica** e **Tipo**. Un clic sull'intestazione ordina per quella colonna (Nome, Dimensione, Data, Tipo); un secondo clic inverte l'ordine. Ogni file ha l'icona del suo tipo. I nomi lunghi sono tagliati: passandoci sopra, un tooltip mostra il percorso completo.

#### Copiare o spostare file

Trascina un file o una cartella da una colonna all'altra. Funziona come in Windows: con `Ctrl` premuto **copia**, con `Maiusc` **sposta**; senza tasti segue la modalità (`Copia` o `Sposta`) scelta nella barra in alto.

Il rilascio porta **sempre** l'elemento nella sua posizione speculare sull'altro lato: stessa cartella, stesso posto. Non conta su quale riga lo lasci cadere — GEMINUS allinea i due lati, non riorganizza il disco.

**Più elementi insieme:** `Ctrl`+click aggiunge o toglie una riga dalla selezione, `Maiusc`+click prende tutte le righe fra l'ultima cliccata e questa. Si seleziona in una colonna sola. Trascinando una riga selezionata porti tutto il gruppo, ognuno nella sua posizione speculare; se nel gruppo c'è una cartella, quello che sta dentro di lei viaggia con lei. Si porta solo quello che vedi: cambiare filtro, cercare o chiudere una cartella toglie dalla selezione le righe che spariscono. `Esc` o un click semplice su un file la azzerano; un click semplice su una cartella la apre o la chiude e lascia la selezione com'è, così puoi prendere righe da più cartelle.

Se dall'altra parte c'è già qualcosa con lo stesso nome, GEMINUS chiede solo quando c'è da perdere qualcosa. Un file che manca si copia; un file uguale (✓) si copia sopra senza domande, perché non cambia niente; una cartella che c'è già non viene sostituita: GEMINUS ci entra e ne completa il contenuto, con le stesse regole per i file che ha dentro.

La domanda arriva solo per i file con lo stesso nome ma **diversi**, perché sovrascriverli cancella per sempre la versione che c'era. Per un file trascinato da solo: **Sovrascrivi**, **Rinomina** o **Annulla**. Per un gruppo o una cartella, prima di cominciare, una finestra sola li elenca tutti, anche quelli dentro le cartelle: **Sovrascrivi tutti**, **Salta questi** (restano come sono, tutto il resto passa), **Uno per uno** (per ciascuno Sovrascrivi, Rinomina o Annulla) o **Annulla** (non parte niente).

Anche i file protetti da scrittura vengono rimpiazzati. L'unico caso in cui non ci riesce è quando è la cartella di destinazione a non lasciarsi scrivere, e allora te lo dice invece di far finta.

L'albero si aggiorna man mano: ogni file copiato, spostato o cancellato cambia subito stato nella vista. Non serve rilanciare il confronto.

**Doppio click** su un file lo apre col programma predefinito del sistema.

I collegamenti (🔗) non si trascinano. E se trascini una cartella che ne contiene, i collegamenti dentro **non vengono copiati**: GEMINUS li salta.

A fine operazione la barra di stato in basso dice com'è andata: copiato, spostato, saltato, annullato, o copia parziale col numero dei file rimasti indietro. Se qualcosa non è andato per intero — una cartella copiata a metà, collegamenti saltati, un'operazione fermata — si apre anche un riepilogo con i conti e la destinazione.

In modalità `Sposta`, se qualche file di una cartella è rimasto indietro, **la cartella di partenza non viene rimossa**: sarebbe l'unico modo di perdere quello che non è passato. La ritrovi dov'era e intera, anche con i file che sono già arrivati dall'altra parte: dalla partenza non si toglie niente, e il riepilogo lo dice.

#### Cancellare file

Seleziona una o più righe e premi `Canc`, oppure tasto destro → **Cestino**. L'elemento va nel Cestino del sistema (*su Linux*, il Cestino/Trash) e da lì lo puoi ripristinare.

`Maiusc`+`Canc` elimina **definitivamente**: niente cestino, nessun ripristino. Lo stesso succede se l'unità non ha un cestino (alcune chiavette USB e cartelle di rete): GEMINUS te lo dice ed elimina definitivamente solo dopo la tua conferma. La conferma nomina l'elemento e dice che **verrà eliminato definitivamente**. Il bottone già pronto è Annulla: per cancellare devi scegliere tu.

Su una cartella la cancellazione porta via **tutto quello che c'è dentro**, sottocartelle comprese, in un colpo solo. La conferma nomina la cartella, non elenca il contenuto: guarda cosa c'è dentro prima di confermare.

Può anche fermarsi a metà — un file protetto, una cartella che non si lascia toccare. In quel caso GEMINUS te lo dice, e la vista si rifà su quello che è rimasto davvero sul disco: quello che vedi dopo è la situazione vera, non quella di prima.

Nell'uso normale la rete è il confronto stesso: un file cancellato per sbaglio dal backup esiste ancora sull'originale, e un confronto più un trascinamento lo rimettono a posto. Quello che non torna è ciò che elimini definitivamente dalla parte in cui era l'unica copia: prima di confermare, guarda su quale colonna sei.

Sui collegamenti (🔗) la cancellazione rimuove il collegamento e non tocca il file o la cartella a cui punta.

#### Menu tasto destro

Tasto destro su un **file o cartella**: Apri, Copia / Sposta nell'altro disco, Rinomina, Copia percorso, Cestino (o Elimina).

Tasto destro su un **blocco Disco A / B**: Apri, Cambia cartella, Confronta, Controlla salute unità, Copia percorso.

#### Scorciatoie da tastiera

| Tasti | Azione |
|---|---|
| `Ctrl`+`C` / `Ctrl`+`X` / `Ctrl`+`V` | Copia / sposta la selezione nell'altro disco |
| `Canc` | Cestino |
| `Maiusc`+`Canc` | Elimina definitivamente |
| `F5` | Ripeti il confronto |
| `F2` | Rinomina |
| `Ctrl`+`F` | Cerca |
| `Ctrl`+`A` | Seleziona tutto |
| `Esc` | Deseleziona |

#### Preferiti

Il menu **★ Preferiti** salva la coppia A↔B corrente con un nome, carica una coppia salvata o la rimuove.

#### Pianificazione

Il pulsante **⏱ Pianificazione** in alto apre la finestra delle attività pianificate: GEMINUS confronta da solo due cartelle negli orari che scegli e, se vuoi, le allinea copiando.

La finestra elenca le attività, ognuna con **nome**, **dischi** (A e B), **quando** parte, **prossima esecuzione** ed **esito dell'ultima esecuzione**. Ogni attività ha quattro pulsanti: **Esegui ora**, **Modifica**, **Registro** ed **Elimina**. Il pulsante **Registro** apre il registro dell'attività, dove ogni esecuzione viene annotata.

**Nuova attività** chiede:

- **Nome** dell'attività.
- **Disco A** e **Disco B**, scelti col solito selettore cartella.
- **Confronto**: **Veloce** o **Approfondito**. Il **Completo** non è pianificabile, perché richiede il permesso di amministratore.
- **Regole sulle differenze**, una per ogni caso:
  - *Solo in A*: copia da A a B / non fare nulla.
  - *Solo in B*: copia da B ad A / non fare nulla.
  - *File diversi*: vince il più recente / copia A→B (sovrascrive B) / copia B→A (sovrascrive A) / non fare nulla.
- **Quando**: ogni giorno a un'ora, ogni settimana in certi giorni a un'ora, oppure ogni N ore (da 1 a 23).

Cosa sapere prima di fidarsi:

- **GEMINUS deve essere aperto.** Le attività partono solo con il programma in esecuzione, anche ridotto a icona. Se la scadenza passa mentre l'app è chiusa, l'attività parte una volta sola all'apertura.
- **Un'attività non cancella mai niente.** Copia soltanto; le regole *non fare nulla* lasciano le differenze dove sono.
- **Prima controlla i dischi.** Prima di copiare, GEMINUS verifica che i dischi siano quelli salvati con l'attività (nome o identificativo dell'unità). Se non corrispondono, l'esecuzione viene saltata e il registro lo annota. Serve con le USB che cambiano lettera quando le colleghi.
- **Un file difettoso non ferma l'esecuzione.** Un file che dà errore viene saltato e annotato nel registro; gli altri vanno avanti.
- **Durante l'esecuzione la finestra è occupata.** Mentre un'attività pianificata è in corso non puoi lanciare dalla finestra altri confronti o copie.

#### Avanzamento nella barra delle applicazioni

Durante confronti e copie l'avanzamento compare anche sul pulsante di GEMINUS nella barra delle applicazioni di Windows.

#### Integrazione con Esplora file (solo Windows)

L'installer aggiunge due voci al menu del tasto destro di ogni cartella o unità in Esplora file: **Imposta come Disco A in GEMINUS** e **Imposta come Disco B in GEMINUS**. Se GEMINUS è già aperto, riusa la finestra. Da riga di comando:

```
geminus.exe --disk-a <percorso> --disk-b <percorso>
```

Il trascinamento da Esplora file sulla finestra di GEMINUS **non** è supportato: usa le voci del menu o il selettore.

#### Quando un file dà problemi

Se durante una copia o uno spostamento un file non si riesce a leggere o scrivere, GEMINUS si ferma e apre una finestra che dice **dov'è l'elemento**, **cosa non è riuscito** (leggere dal disco di partenza, scrivere su quello di destinazione, creare una cartella, leggere il contenuto di una cartella) e **perché**: i permessi non lo consentono, lo spazio è esaurito, il disco di destinazione è in sola lettura, il file è aperto in un altro programma, il disco non risponde più, il disco segnala un errore fisico. In coda c'è anche il testo grezzo del sistema, per chi lo vuole leggere.

Poi ti chiede cosa fare: **Riprova** (tenta di nuovo lo stesso file, utile se il blocco era temporaneo), **Salta** (lascia perdere quel file e va avanti con gli altri), **Annulla** (ferma tutto subito). I file già copiati prima dell'errore restano dove sono, e quello che non è riuscito non lascia niente in destinazione: quello che c'era prima è ancora lì.

Una cartella che ha avuto un file saltato dentro non viene mostrata come allineata: il suo stato lo decide il contenuto, non l'esito della copia.

#### Verifica Salute Estesa

Appena scegli un disco, sul suo blocco compare l'icona 🩺: è la **Verifica Salute Estesa**, e la puoi lanciare quando vuoi — non serve che il controllo rapido abbia segnalato qualcosa.

GEMINUS avvia il test interno del disco, quello che i dischi chiamano SMART: si autodiagnostica il disco stesso, GEMINUS aspetta. Il disco resta utilizzabile durante il test, ma più lento, e puoi annullare in qualsiasi momento.

Quanto ci vuole lo dichiara il disco, e possono essere ore. Quella durata GEMINUS la legge quando fa un confronto **Completo**: se su quel disco ne hai già fatto uno, la finestra ti dice quanto manca e ti avvisa se il test sta sforando la stima; se non ne hai mai fatti, ti dice solo da quanto tempo sta lavorando.

A test finito il risultato viene salvato come file di testo **nella cartella dei download**, e GEMINUS ti dice il percorso esatto.

Anche qui serve il permesso, ma qui lo chiede direttamente il sistema — nel Completo era GEMINUS a chiedertelo prima con una sua finestra. Se lo rifiuti, la verifica non parte e non succede nient'altro. E se il programma esterno non è installato non c'è niente da avviare: la finestra ti spiega come installarlo, e da lì si torna indietro qualunque bottone premi.

#### Annullare un'operazione

Durante la scansione e il confronto c'è sempre una finestra col bottone `Annulla`. Nelle copie e negli spostamenti compare quando c'è qualcosa da aspettare — una cartella o un file grande: per un file piccolo l'operazione è finita prima che serva fermarla.

L'annullamento è pulito: la copia a metà viene buttata via e il file che stava in destinazione resta esattamente com'era, intatto. Quelli già finiti restano dove sono, e la vista mostra la situazione vera — non quella che ci si aspettava prima di fermarsi.

---

### Dischi lenti o difettosi

#### Cosa sono i settori difettosi

Ogni disco è diviso fisicamente in piccole zone chiamate **settori**. Col tempo, alcune zone smettono di funzionare correttamente (settori difettosi, in inglese *bad sectors*). Quando GEMINUS prova a leggere un file che si trova su un settore difettoso, il disco può rispondere lentamente, dare errore, o non rispondere affatto.

#### Messaggio "In attesa del disco..."

Compare durante la lettura del contenuto dei file quando il disco non risponde da qualche secondo. **Non è un errore**: GEMINUS sta aspettando con pazienza. Se il disco si sblocca, la barra torna a "Lettura contenuto dei file..." e prosegue. Cosa fare: niente, lascia lavorare. Cosa NON fare: non chiudere la finestra, non scollegare il disco.

Questo non capita in modalità Veloce: in Veloce GEMINUS non legge il contenuto dei file, quindi non resta mai in attesa del disco.

#### Contatore "illeggibili"

A fine confronto, nello status: `(N Illeggibili)`. Sono i file che GEMINUS non è riuscito a leggere: il disco non ha risposto entro il tempo limite, oppure ha dato errore, oppure i permessi non lo consentivano. GEMINUS li ha marcati come illeggibili e ha continuato col file successivo. Filtra l'albero col chip `Illeggibili` per vedere quali sono.

#### Contatore "saltati per disco bloccato"

Compare solo nei casi gravi: `(N saltati per disco bloccato)`. Significa che il disco si è bloccato così tante volte (8 file falliti) che GEMINUS ha smesso di insistere e ha saltato i file rimanenti per non sprecare tempo e risorse.

#### Cosa fare quando vedi questi numeri

- **Pochi illeggibili (1-3)**: probabilmente settori difettosi isolati. Fai un backup urgente dei file ancora leggibili su un altro disco.
- **Molti illeggibili o saltati**: il disco è seriamente compromesso. Recupera tutto il salvabile, poi vai alla sezione [Formattare un disco](#formattare-un-disco) per riformattare il disco con controllo settori, oppure sostituiscilo se è in fine vita.
- **Disco con molti settori in attesa di rimappatura** (decine o più): considera di riformattarlo col controllo dei settori attivo, così quelli difettosi vengono marcati e il sistema non li usa più. Come si fa, sui due sistemi, è nel capitolo [Formattare un disco](#formattare-un-disco).

---

### Formattare un disco

> ⚠ **Attenzione**: formattare un disco cancella tutti i dati. Verifica DUE VOLTE quale disco stai formattando prima di procedere. Sbagliare disco significa perdere tutto.

#### Prima di tutto — salvare i dati recuperabili

**Prima di formattare, copia altrove tutto ciò che è ancora leggibile.** La formattazione cancella tutto, anche dai settori sani. Usa GEMINUS o il gestore file del sistema per copiare i dati su un altro disco. Solo quando sei sicuro di non avere più nulla da salvare, procedi.

Da qui in avanti le due strade sono diverse: la formattazione è una cosa che fa il sistema, non GEMINUS.

#### Su Windows

Serve solo Esplora file.

1. Apri **Esplora file** e trova l'unità nell'elenco di sinistra.
2. Tasto destro sull'unità → **Formatta**.
3. Scegli il file system: **NTFS** se il disco resterà su Windows, **exFAT** se lo userai anche su Linux o Mac (regge file di qualsiasi dimensione), **FAT32** solo per apparecchi vecchi che non capiscono gli altri due — un file singolo non può superare i 4 GB, e Windows lo propone solo per le unità piccole.
4. **Togli la spunta a "Formattazione veloce"**. È il punto che conta: quella veloce non guarda il disco, quella completa passa su tutti i settori e marca i difettosi, così il sistema non li usa più. Su un disco grande può richiedere ore.
5. Premi **Avvia** e aspetta la fine.

Se invece vuoi solo controllare un disco malato **senza cancellare niente**, apri il Prompt dei comandi come amministratore e dai `chkdsk X: /r`, con `X` al posto della lettera dell'unità: cerca i settori danneggiati e recupera quello che riesce a leggere. Anche questo può richiedere ore.

#### Su Linux

Qui si passa dal terminale, e i comandi vanno dati con la testa: identifica il disco prima di toccarlo.

##### Passo 1 — identificare il disco

Apri un terminale e dai:

```bash
lsblk -f
```

Riconosci il tuo disco USB dalla dimensione e dall'etichetta. Esempio: una USB da 4TB potrebbe essere `/dev/sdb` con la sua partizione `/dev/sdb1`. **Mai usare `/dev/sda`** senza essere sicurissimi: di solito è il disco di sistema.

##### Passo 2 — smontare

```bash
sudo umount /dev/sdX1
```

Sostituisci `X` con la lettera reale (es. `sdb1`). Se il disco non era montato, ignora l'errore.

##### Passo 3 — formattare (scegli il filesystem)

**ext4 — raccomandato per uso Linux, con controllo settori:**

```bash
sudo mkfs.ext4 -c -L "MioDisco" /dev/sdX1
```

L'opzione `-c` esegue un controllo letture single-pass durante la formattazione. Per un controllo più approfondito (letture *e* scritture, molto più lento, ore su dischi grandi):

```bash
sudo mkfs.ext4 -cc -L "MioDisco" /dev/sdX1
```

**ext4 con badblocks pre-scansione (massimo rigore):**

```bash
sudo badblocks -wsv -o badblocks.txt /dev/sdX1
sudo mkfs.ext4 -l badblocks.txt -L "MioDisco" /dev/sdX1
```

`badblocks -wsv` sovrascrive ogni settore con 4 pattern e verifica. Distrugge tutti i dati. Salva la lista dei settori cattivi che `mkfs.ext4 -l` esclude dal filesystem.

**FAT32 — compatibilità universale (Windows, Mac, Linux, console). Limite: file singoli max 4 GB.**

```bash
sudo mkfs.vfat -F 32 -n MIODISCO /dev/sdX1
```

L'etichetta in FAT è max 11 caratteri maiuscoli.

**NTFS — Windows, supporta file grandi. Richiede pacchetto `ntfs-3g`.**

```bash
sudo mkfs.ntfs -Q -L "MioDisco" /dev/sdX1
```

`-Q` = quick format. Senza `-Q` il check è più approfondito ma richiede ore.

**exFAT — compatibilità Windows/Mac, file >4 GB. Richiede `exfatprogs`.**

```bash
sudo mkfs.exfat -L "MioDisco" /dev/sdX1
```

##### Passo 4 — rimontare

Scollega fisicamente la USB e ricollegala: il gestore file la rimonterà in automatico. In alternativa:

```bash
sudo mkdir -p /mnt/miodisco
sudo mount /dev/sdX1 /mnt/miodisco
```

##### Passo 5 — verifica

```bash
df -h | grep sdX
```

Controlla che lo spazio totale sia quello atteso e che il filesystem sia quello giusto.

---

### Domande frequenti

**GEMINUS modifica i miei file?**
Non da solo. Il confronto è in sola lettura: guarda e non tocca niente. I file cambiano solo quando lo chiedi tu, e sono due i modi: trascinare qualcosa da una colonna all'altra, o eliminare (tasto destro o `Canc`). Nel primo caso, se dall'altra parte c'è un file con lo stesso nome ma diverso, GEMINUS chiede sempre cosa fare prima di sovrascriverlo; nel secondo l'elemento va nel Cestino, a meno che tu non scelga l'eliminazione definitiva (che chiede conferma).

**I miei dati vengono inviati su internet?**
No. GEMINUS funziona completamente offline. Non c'è telemetria, non c'è cloud, non c'è connessione di rete. Tutto resta sul tuo computer.

**Quanto tempo ci vuole per un confronto?**
Veloce è veloce, anche su dischi grandi. Approfondito può richiedere ore su dischi grandi, perché legge il contenuto dei file. Completo è come Approfondito, più la verifica della salute. Se il disco ha problemi, i tempi si allungano.

**Posso usare GEMINUS su Windows o Mac?**
Su Linux e su Windows sì — su Windows serve la versione 10 22H2 o successiva, a 64 bit. Su Mac no.

**Cosa fa esattamente "Verifica Salute Estesa"?**
Avvia il test interno del disco: si autodiagnostica il disco stesso, GEMINUS aspetta e riporta l'esito. La durata la dichiara il disco e può essere di ore. Il risultato viene salvato come file di testo nella cartella dei download.

**Posso annullare durante un confronto lungo?**
Sì. La finestra del confronto ha sempre un bottone **Annulla**. L'app si ferma in modo pulito, senza lasciare file a metà.

**Perché alcune cartelle sono "escluse"?**
GEMINUS salta le cartelle che cambiano continuamente da sole, dove il confronto non direbbe niente di utile e costerebbe tempo: `.git`, `node_modules`, `target`, `.cargo`, `.rustup`, `.venv`, `.cache`, `__pycache__`, cestino. *Su Windows* salta anche le cartelle che il sistema tiene per sé (`$RECYCLE.BIN`, `System Volume Information`, `Config.Msi`), quelle che stanno nella radice di ogni unità: non sono tue, e confrontarle non dice niente sul tuo backup. Conta il nome: una cartella tua chiamata così verrebbe saltata comunque. Vale per il confronto, non per la copia: se trascini una cartella che ne contiene una di queste, viene copiata per intero — copiare una cartella deve darti la cartella, non una versione con dei pezzi in meno.

**Quanto è affidabile il confronto Approfondito?**
Molto. GEMINUS legge il contenuto dei due file dall'inizio alla fine: se differiscono anche di un solo byte, lo vede. Il caso in cui due file diversi vengano dichiarati uguali è così improbabile che non capiterà in nessuna vita umana.

**Dove finiscono i file cancellati?**
Nel Cestino del sistema (*su Linux*, il Cestino/Trash), da dove li puoi ripristinare. Solo `Maiusc`+`Canc`, oppure un'unità senza cestino e dopo la tua conferma, elimina per sempre.

**Posso trascinare file da Esplora file dentro GEMINUS?**
No. Usa le voci del tasto destro "Imposta come Disco A / B in GEMINUS" oppure il selettore cartella.

**Posso pianificare con il programma chiuso?**
No, per ora serve l'app aperta (anche ridotta a icona va bene). Se la scadenza passa mentre è chiusa, l'attività parte una volta sola all'apertura. Vedi [Pianificazione](#pianificazione).
