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

### Table Of Contents

1. [About GEMINUS](#about-geminus)
2. [How To Use](#how-to-use)
3. [Slow Or Faulty Disks](#slow-or-faulty-disks)
4. [Formatting A Disk](#formatting-a-disk)
5. [Frequently Asked Questions](#frequently-asked-questions)

---

### About GEMINUS

#### What GEMINUS Is

GEMINUS is a desktop application that **compares two disks or two folders**. It shows side by side which files are identical, which have been modified, which exist on only one side.

#### Where It Runs

On Linux and on Windows, with the same features. On Windows it needs version 10 22H2 or later, 64 bit.

The few things that differ between the two systems are marked throughout this manual with **On Linux** and **On Windows**. Where there's no marker, it holds for both.

#### What It's For

- Verify that a backup is complete and identical to the source.
- Find modified files between two copies of a folder.
- Discover whether an old disk holds files missing from a new one.
- Copy or move files and folders between sides by dragging them.
- Delete from the backup what no longer exists in the source.

#### How It Works

When you click **▶ Compare**, GEMINUS asks how you want to compare the disks. There are three methods:

- **Quick**. GEMINUS looks at file names, sizes and dates. Fast, but if two files have the same size and the same date it treats them as identical even if they're different inside.
- **Deep**. GEMINUS also reads the contents of files that look the same (same name, same size, same date), to be sure they really are identical inside. Files that differ it spots right away without reading them. Slow, but reliable.
- **Full**. Like Deep, plus it checks the physical health of the disks before starting. To do that it asks for permission: *on Linux* the administrator password, *on Windows* your consent in the system's own window.

#### When To Use Which Method

For everyday use (seeing what changed after a copy): **Quick**.

For checking an important backup (years of photos, documents that really matter to you): **Deep**.

When you have doubts about the disk (old USB drive, suspicion that something isn't right): **Full**.

#### Technology

GEMINUS is written in Rust on the Tauri framework. It is free software distributed under GPL v3. Your data always stays on your computer: nothing is sent over the network.

---

### How To Use

#### Basic Compare

1. Click on the **Drive A** block in the top left. The folder picker opens: `⏏ Devices` lists the connected disks, `🏠 Home` takes you to your personal folder. Pick a folder and press **Select This Folder**.
2. Same with the **Drive B** block on the right.
3. Click **▶ Compare** in the center. GEMINUS asks which method to use: **Quick**, **Deep** or **Full**. Choose, read the confirmation, press **Proceed**.

The list that appears under `⏏ Devices` is not a folder and cannot be confirmed: go into a disk and choose in there. With no external disks connected that list is empty: the root and your personal folder are not in there, they have buttons of their own.

*On Linux* the picker also has `/ Root`, the root of the filesystem. *On Windows* that button isn't there, because a single root doesn't exist: the disks are the drives (`C:`, `D:`, …) and you find them under `⏏ Devices`.

If you chose **Full**, GEMINUS asks for permission to check the disks' health — *on Linux* the administrator password, *on Windows* your consent in the system's own window. If you don't grant it, you can carry on with the Deep Comparison alone or go back to the method selection.

The health check relies on a free external program, installed once and for all. If it isn't there, GEMINUS tells you and explains how to install it the way your system does it; you can also carry on without, and then Full counts as Deep. Comparing and copying don't depend on that program: they work either way.

#### What You See After The Compare

The two side-by-side trees show the contents of the two disks, row by row: where an item is missing on one side, an **(absent)** row takes its place, so the columns stay aligned at any depth. Rows are color-coded by status: **blue** = only in A, **green** = only in B, **orange** = modified, and a badge labels each one. The status bar at the bottom summarizes the counts.

Folders reflect the state of what's inside them: if even one file within a folder is different or missing on one side, the folder itself is marked Modified. Expand it to see what's changed.

After a **Quick** comparison a warning stays above the trees, and for good reason: that comparison did not read file contents, so there "identical" means "same size and same date".

#### Filters And Search

The chips at the top filter by status: `All`, `Modified`, `Only In A`, `Only In B`, `Same In A And B`, `Unreadable`. Search filters by name.

The `Hidden` chip also shows the items your system considers hidden: *on Linux* those whose name starts with a dot, *on Windows* those carrying the hidden attribute. An item hidden on one side and plain on the other stays visible anyway, because that is a difference. Worth knowing: the same disk compared from Linux and from Windows can show a different number of hidden files — what changes is the notion of hidden, not the disk.

The numbers on the chips count only files. Folders appear colored in the tree but are not part of the totals.

#### Copy Or Move Files

Drag a file or a folder from one column to the other. The mode (`Copy` or `Move`) is chosen in the toolbar.

Dropping **always** puts the item in its mirror position on the other side: same folder, same place. It doesn't matter which row you drop it on — GEMINUS aligns the two sides, it doesn't rearrange the disk.

If something with the same name is already on the other side, GEMINUS asks what to do: **Overwrite**, **Rename** or **Cancel**. On a folder, Overwrite does not replace it: it merges the contents, replaces the files with the same name, and leaves the rest alone.

Write-protected files get replaced too. The one case where it can't is when the destination folder itself refuses to be written to, and then it tells you instead of pretending.

The tree updates as you go: every file copied, moved, or deleted changes status in the view right away. There's no need to run the compare again.

**Double click** on a file opens it with the system's default program.

Links (🔗) can't be dragged. And if you drag a folder that contains some, the links inside **are not copied**: GEMINUS skips them.

When an operation ends, the status bar at the bottom says how it went: copied, moved, skipped, cancelled, or partial copy with the number of files left behind. If something didn't go through in full — a folder copied halfway, links skipped, an operation stopped — a summary also opens, with the counts and the destination.

In `Move` mode, if any file of a folder was left behind, **the source folder is not removed**: that would be the one way to lose what didn't make it. You find it where it was, with inside what didn't get through.

#### Delete Files

**Right click** on a row → **Delete**. GEMINUS asks for confirmation, naming the item and saying it **will be permanently deleted**. The button already primed is Cancel: to delete, you have to choose it yourself.

On a folder, deleting takes away **everything inside**, subfolders included, in one go. The confirmation names the folder, it doesn't list the contents: look at what's in there before you confirm.

It can also stop halfway — a protected file, a folder that will not be touched. GEMINUS says so, and the view is rebuilt on what is actually still on the disk: what you see afterwards is the real situation, not the one from before.

And permanently means permanently: **no recycle bin, no undo**. It's a choice, not an oversight — the system's recycle bin, from here, moved nothing and warned about nothing, and a recycle bin inside the app would have promised a safety it couldn't keep. Better a clear confirmation than a net that doesn't hold.

In normal use the net is the comparison itself: a file deleted from the backup by mistake still exists in the source, and one comparison plus one drag puts it back. What doesn't come back is what you delete from the side where it was the only copy: before confirming, look at which column you're on.

On links (🔗), deleting removes the link and doesn't touch the file or folder it points to.

#### When A File Has Trouble

If during a copy or move a file can't be read or written, GEMINUS stops and opens a window that says **where the item is**, **what failed** (reading from the source disk, writing to the destination disk, creating a folder, reading the contents of a folder) and **why**: permissions don't allow it, space has run out, the destination disk is read-only, the file is open in another program, the disk stopped responding, the disk reports a physical error. The system's raw text follows at the end, for whoever wants to read it.

Then it asks what to do: **Retry** (try the same file again, useful if the block was temporary), **Skip** (leave that file and move on with the rest), **Cancel** (stop everything now). Files already copied before the error stay where they are, and the one that failed leaves nothing behind on the destination side: what was there before is still there.

A folder that had a file skipped inside it is not shown as aligned: its status is decided by its contents, not by how the copy ended.

#### Extended Health Check

As soon as you pick a disk, the 🩺 icon appears on its block: that's the **Extended Health Check**, and you can launch it whenever you want — the quick check doesn't need to have flagged anything.

GEMINUS starts the disk's internal test, the one disks call SMART: the disk diagnoses itself, GEMINUS waits. The disk stays usable during the test, but slower, and you can cancel at any time.

How long it takes is declared by the disk, and it can be hours. GEMINUS reads that figure when it runs a **Full** comparison: if you've already run one on that disk, the window tells you how much is left and warns you if the test is overrunning its estimate; if you never have, it only tells you how long it's been working.

When the test is over the result is saved as a text file **in your downloads folder**, and GEMINUS tells you the exact path.

Permission is needed here too, but here the system asks for it directly — in Full it was GEMINUS asking you first, in its own window. If you refuse, the check doesn't start and nothing else happens. And if the external program isn't installed there's nothing to start: the window explains how to install it, and from there you go back whichever button you press.

#### Cancel An Operation

During the scan and the comparison there's always a window with a `Cancel` button. In copies and moves it appears when there's something to wait for — a folder or a large file: for a small file the operation is over before stopping it would matter.

Cancelling is clean: the copy in progress is thrown away and the file that was already on the destination side stays exactly as it was, untouched. The ones already finished stay where they are, and the view shows the real situation — not the one expected before stopping.

---

### Slow Or Faulty Disks

#### What Bad Sectors Are

Every disk is physically divided into small zones called **sectors**. Over time, some zones stop working correctly (bad sectors). When GEMINUS tries to read a file located on a bad sector, the disk may respond slowly, return an error, or not respond at all.

#### Message "Waiting For Disk..."

Appears while reading file contents when the disk hasn't responded for a few seconds. **It's not an error**: GEMINUS is waiting patiently. If the disk unblocks, the bar reverts to "Reading File Contents..." and continues. What to do: nothing, let it work. What NOT to do: don't close the window, don't unplug the disk.

This doesn't happen in Quick mode: in Quick mode GEMINUS doesn't read file contents, so it never has to wait for the disk.

#### Counter "Unreadable"

At the end of the compare, in the status: `(N Unreadable)`. These are the files GEMINUS could not read: the disk did not answer within the time limit, or returned an error, or permissions did not allow it. GEMINUS marked them unreadable and moved on. Filter the tree with the `Unreadable` chip to see them.

#### Counter "Skipped — Disk Stuck"

Appears only in serious cases: `(N Skipped — Disk Stuck)`. Means the disk got stuck so many times (8 failed files) that GEMINUS gave up and skipped the remaining files to avoid wasting time and resources.

#### What To Do When You See These Numbers

- **Few Unreadable (1-3)**: probably isolated bad sectors. Make an urgent backup of the still-readable files to another disk.
- **Many Unreadable or Skipped**: the disk is seriously compromised. Salvage what you can, then go to the [Formatting A Disk](#formatting-a-disk) section to reformat with sector check, or replace the disk if it's at end of life.
- **Disk With Many Sectors Waiting To Be Remapped** (dozens or more): consider reformatting it with sector checking on, so the faulty ones get marked and the system stops using them. How to do it, on both systems, is in the [Formatting A Disk](#formatting-a-disk) chapter.

---

### Formatting A Disk

> ⚠ **Warning**: Formatting a disk wipes all data. Double-check which disk you are formatting before you proceed. Picking the wrong disk means losing everything.

#### First Of All — Save What Can Be Recovered

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

##### Step 1 — Identify The Disk

Open a terminal and run:

```bash
lsblk -f
```

Recognize your USB by size and label. Example: a 4 TB USB might be `/dev/sdb` with partition `/dev/sdb1`. **Never use `/dev/sda`** unless you're absolutely sure: it's usually the system disk.

##### Step 2 — Unmount

```bash
sudo umount /dev/sdX1
```

Replace `X` with the actual letter (e.g. `sdb1`). If the disk wasn't mounted, ignore the error.

##### Step 3 — Format (Pick A Filesystem)

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

##### Step 4 — Remount

Physically unplug the USB and plug it back in: the file manager will auto-mount it. Alternatively:

```bash
sudo mkdir -p /mnt/mydisk
sudo mount /dev/sdX1 /mnt/mydisk
```

##### Step 5 — Verify

```bash
df -h | grep sdX
```

Check that the total space matches expectations and that the filesystem is the right one.

---

### Frequently Asked Questions

**Does GEMINUS modify my files?**
Not on its own. The compare is read-only: it looks and touches nothing. Files change only when you ask, and there are two ways: dragging something from one column to the other, or deleting with the right button. In the first case, if there's a conflict, GEMINUS always asks what to do; in the second it asks for confirmation, and the deletion is permanent.

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

---
---

## Manuale — Italiano

**[English](#english)** · **Italiano**

### Indice

1. [Cosa È GEMINUS](#cosa-è-geminus)
2. [Come Si Usa](#come-si-usa)
3. [Dischi Lenti O Difettosi](#dischi-lenti-o-difettosi)
4. [Formattare Un Disco](#formattare-un-disco)
5. [Domande Frequenti](#domande-frequenti)

---

### Cosa È GEMINUS

#### Cosa È

GEMINUS è un'applicazione desktop che **confronta due dischi o due cartelle**. Mostra fianco a fianco quali file sono uguali, quali sono stati modificati, quali esistono solo da una parte.

#### Dove Gira

Su Linux e su Windows, con le stesse funzioni. Su Windows serve la versione 10 22H2 o successiva, a 64 bit.

Le poche cose che cambiano da un sistema all'altro sono segnalate lungo il manuale con **Su Linux** e **Su Windows**. Dove non c'è nessuna indicazione, vale per tutti e due.

#### A Cosa Serve

- Verificare che un backup sia completo e identico all'originale.
- Trovare i file modificati fra due copie di una cartella.
- Capire se un disco vecchio contiene file che mancano in quello nuovo.
- Copiare o spostare file e cartelle da una parte all'altra trascinandoli.
- Cancellare dal backup quello che nell'originale non c'è più.

#### Come Lavora

Quando clicchi **▶ Confronta**, GEMINUS ti chiede in che modo vuoi confrontare i dischi. Ci sono tre metodi:

- **Veloce**. GEMINUS guarda i nomi dei file, le dimensioni e le date. Rapido, ma se due file hanno stessa dimensione e stessa data li considera uguali anche se dentro sono diversi.
- **Approfondito**. GEMINUS legge anche il contenuto dei file che sembrano uguali (stesso nome, stessa dimensione, stessa data), per essere sicuro che siano davvero identici dentro. I file diversi li riconosce subito senza leggerli. Lento, ma sicuro.
- **Completo**. Come Approfondito, e in più controlla lo stato di salute fisica dei dischi prima di partire. Per farlo chiede un permesso: *su Linux* la password di amministratore, *su Windows* il consenso nella finestra del sistema.

#### Quando Usare Quale Metodo

Per uso quotidiano (vedere cos'è cambiato dopo una copia): **Veloce**.

Per verificare un backup importante (le foto di anni, documenti che ti servono davvero): **Approfondito**.

Quando hai dubbi sul disco (USB vecchia, sospetto che qualcosa non vada): **Completo**.

#### Tecnologia

GEMINUS è scritto in Rust col framework Tauri. È software libero distribuito sotto licenza GPL v3. I dati restano sempre sul tuo computer: niente viene inviato in rete.

---

### Come Si Usa

#### Confronto Base

1. Clicca sul blocco **Disco A** in alto a sinistra. Si apre il selettore cartella: `⏏ Dispositivi` elenca i dischi collegati, `🏠 Home` porta alla tua cartella personale. Scegli una cartella e premi **Scegli Questa Cartella**.
2. Stessa cosa col blocco **Disco B** a destra.
3. Clicca **▶ Confronta** al centro. GEMINUS ti chiede quale metodo usare: **Veloce**, **Approfondito** o **Completo**. Scegli, leggi la conferma, premi **Procedi**.

L'elenco che appare sotto `⏏ Dispositivi` non è una cartella e non si può confermare: entra in un disco e scegli lì dentro. Se non ci sono dischi esterni collegati quell'elenco resta vuoto: la radice e la tua cartella personale non stanno lì, hanno i loro pulsanti.

*Su Linux* il selettore ha anche `/ Root`, la radice del sistema. *Su Windows* quel pulsante non c'è, perché una radice unica non esiste: i dischi sono le unità (`C:`, `D:`, …) e le trovi sotto `⏏ Dispositivi`.

Se hai scelto **Completo**, GEMINUS chiede il permesso di controllare la salute dei dischi — *su Linux* la password di amministratore, *su Windows* il consenso nella finestra del sistema. Se non lo dai, puoi continuare col solo Confronto Approfondito o tornare alla scelta del metodo.

Il controllo della salute si appoggia a un programma esterno gratuito, che si installa una volta sola. Se non lo trova, GEMINUS te lo dice e ti spiega come installarlo nel modo del tuo sistema; puoi anche continuare senza, e allora il Completo vale come Approfondito. Confronto e copia non dipendono da quel programma: funzionano comunque.

#### Cosa Vedi Dopo Il Confronto

I due alberi affiancati mostrano il contenuto dei due dischi, riga per riga: dove un elemento manca da una parte, al suo posto compare la riga **(assente)**, così le due colonne restano allineate a qualsiasi profondità. Le righe sono colorate per stato: **azzurro** = solo in A, **verde** = solo in B, **arancione** = modificato, e un badge le etichetta una per una. La barra in basso riassume i conteggi.

Le cartelle riflettono lo stato di ciò che contengono: se anche un solo file dentro una cartella è diverso o manca da una parte, la cartella stessa è marcata come Modificata. Espandila per vedere cosa cambia.

Dopo un confronto **Veloce** resta un avviso sopra gli alberi, e ha un motivo: quel confronto non ha letto il contenuto dei file, quindi lì "uguale" significa "stessa dimensione e stessa data".

#### Filtri E Ricerca

I chip in alto filtrano per stato: `Tutti`, `Modificati`, `Solo in A`, `Solo in B`, `Uguali in A e B`, `Illeggibili`. La ricerca filtra per nome.

Il chip `Nascosti` mostra anche gli elementi che il tuo sistema considera nascosti: *su Linux* quelli col nome che inizia per punto, *su Windows* quelli che portano l'attributo nascosto. Un elemento nascosto da una parte e normale dall'altra resta visibile comunque, perché quella è una differenza. Conseguenza da sapere: lo stesso disco confrontato da Linux e da Windows può mostrare un numero diverso di file nascosti — è la nozione di nascosto che cambia, non il disco.

I numeri sui chip contano solo i file. Le cartelle si vedono colorate nell'albero ma non rientrano nei totali.

#### Copiare O Spostare File

Trascina un file o una cartella da una colonna all'altra. La modalità (`Copia` o `Sposta`) si sceglie nella barra in alto.

Il rilascio porta **sempre** l'elemento nella sua posizione speculare sull'altro lato: stessa cartella, stesso posto. Non conta su quale riga lo lasci cadere — GEMINUS allinea i due lati, non riorganizza il disco.

Se dall'altra parte c'è già qualcosa con lo stesso nome, GEMINUS chiede cosa fare: **Sovrascrivi**, **Rinomina** o **Annulla**. Su una cartella, Sovrascrivi non la sostituisce: ne fonde il contenuto, rimpiazza i file con lo stesso nome e lascia stare il resto.

Anche i file protetti da scrittura vengono rimpiazzati. L'unico caso in cui non ci riesce è quando è la cartella di destinazione a non lasciarsi scrivere, e allora te lo dice invece di far finta.

L'albero si aggiorna man mano: ogni file copiato, spostato o cancellato cambia subito stato nella vista. Non serve rilanciare il confronto.

**Doppio click** su un file lo apre col programma predefinito del sistema.

I collegamenti (🔗) non si trascinano. E se trascini una cartella che ne contiene, i collegamenti dentro **non vengono copiati**: GEMINUS li salta.

A fine operazione la barra di stato in basso dice com'è andata: copiato, spostato, saltato, annullato, o copia parziale col numero dei file rimasti indietro. Se qualcosa non è andato per intero — una cartella copiata a metà, collegamenti saltati, un'operazione fermata — si apre anche un riepilogo con i conti e la destinazione.

In modalità `Sposta`, se qualche file di una cartella è rimasto indietro, **la cartella di partenza non viene rimossa**: sarebbe l'unico modo di perdere quello che non è passato. La ritrovi dov'era, con dentro quello che non ce l'ha fatta.

#### Cancellare File

**Tasto destro** su una riga → **Cancella**. GEMINUS chiede conferma dicendo il nome dell'elemento e che **verrà eliminato definitivamente**. Il bottone già pronto è Annulla: per cancellare devi scegliere tu.

Su una cartella la cancellazione porta via **tutto quello che c'è dentro**, sottocartelle comprese, in un colpo solo. La conferma nomina la cartella, non elenca il contenuto: guarda cosa c'è dentro prima di confermare.

Può anche fermarsi a metà — un file protetto, una cartella che non si lascia toccare. In quel caso GEMINUS te lo dice, e la vista si rifà su quello che è rimasto davvero sul disco: quello che vedi dopo è la situazione vera, non quella di prima.

E definitivamente vuol dire definitivamente: **niente cestino, nessun ripristino**. È una scelta, non una dimenticanza — il cestino del sistema, da qui, non spostava niente e non avvisava, e un cestino interno all'app avrebbe promesso una sicurezza che non poteva mantenere. Meglio una conferma chiara di una rete che non tiene.

Nell'uso normale la rete è il confronto stesso: un file cancellato per sbaglio dal backup esiste ancora sull'originale, e un confronto più un trascinamento lo rimettono a posto. Quello che non torna è ciò che cancelli dalla parte in cui era l'unica copia: prima di confermare, guarda su quale colonna sei.

Sui collegamenti (🔗) la cancellazione rimuove il collegamento e non tocca il file o la cartella a cui punta.

#### Quando Un File Dà Problemi

Se durante una copia o uno spostamento un file non si riesce a leggere o scrivere, GEMINUS si ferma e apre una finestra che dice **dov'è l'elemento**, **cosa non è riuscito** (leggere dal disco di partenza, scrivere su quello di destinazione, creare una cartella, leggere il contenuto di una cartella) e **perché**: i permessi non lo consentono, lo spazio è esaurito, il disco di destinazione è in sola lettura, il file è aperto in un altro programma, il disco non risponde più, il disco segnala un errore fisico. In coda c'è anche il testo grezzo del sistema, per chi lo vuole leggere.

Poi ti chiede cosa fare: **Riprova** (tenta di nuovo lo stesso file, utile se il blocco era temporaneo), **Salta** (lascia perdere quel file e va avanti con gli altri), **Annulla** (ferma tutto subito). I file già copiati prima dell'errore restano dove sono, e quello che non è riuscito non lascia niente in destinazione: quello che c'era prima è ancora lì.

Una cartella che ha avuto un file saltato dentro non viene mostrata come allineata: il suo stato lo decide il contenuto, non l'esito della copia.

#### Verifica Salute Estesa

Appena scegli un disco, sul suo blocco compare l'icona 🩺: è la **Verifica Salute Estesa**, e la puoi lanciare quando vuoi — non serve che il controllo rapido abbia segnalato qualcosa.

GEMINUS avvia il test interno del disco, quello che i dischi chiamano SMART: si autodiagnostica il disco stesso, GEMINUS aspetta. Il disco resta utilizzabile durante il test, ma più lento, e puoi annullare in qualsiasi momento.

Quanto ci vuole lo dichiara il disco, e possono essere ore. Quella durata GEMINUS la legge quando fa un confronto **Completo**: se su quel disco ne hai già fatto uno, la finestra ti dice quanto manca e ti avvisa se il test sta sforando la stima; se non ne hai mai fatti, ti dice solo da quanto tempo sta lavorando.

A test finito il risultato viene salvato come file di testo **nella cartella dei download**, e GEMINUS ti dice il percorso esatto.

Anche qui serve il permesso, ma qui lo chiede direttamente il sistema — nel Completo era GEMINUS a chiedertelo prima con una sua finestra. Se lo rifiuti, la verifica non parte e non succede nient'altro. E se il programma esterno non è installato non c'è niente da avviare: la finestra ti spiega come installarlo, e da lì si torna indietro qualunque bottone premi.

#### Annullare Un'Operazione

Durante la scansione e il confronto c'è sempre una finestra col bottone `Annulla`. Nelle copie e negli spostamenti compare quando c'è qualcosa da aspettare — una cartella o un file grande: per un file piccolo l'operazione è finita prima che serva fermarla.

L'annullamento è pulito: la copia a metà viene buttata via e il file che stava in destinazione resta esattamente com'era, intatto. Quelli già finiti restano dove sono, e la vista mostra la situazione vera — non quella che ci si aspettava prima di fermarsi.

---

### Dischi Lenti O Difettosi

#### Cosa Sono I Settori Difettosi

Ogni disco è diviso fisicamente in piccole zone chiamate **settori**. Col tempo, alcune zone smettono di funzionare correttamente (settori difettosi, in inglese *bad sectors*). Quando GEMINUS prova a leggere un file che si trova su un settore difettoso, il disco può rispondere lentamente, dare errore, o non rispondere affatto.

#### Messaggio "In Attesa Del Disco..."

Compare durante la lettura del contenuto dei file quando il disco non risponde da qualche secondo. **Non è un errore**: GEMINUS sta aspettando con pazienza. Se il disco si sblocca, la barra torna a "Lettura Contenuto Dei File..." e prosegue. Cosa fare: niente, lascia lavorare. Cosa NON fare: non chiudere la finestra, non scollegare il disco.

Questo non capita in modalità Veloce: in Veloce GEMINUS non legge il contenuto dei file, quindi non resta mai in attesa del disco.

#### Contatore "Illeggibili"

A fine confronto, nello status: `(N Illeggibili)`. Sono i file che GEMINUS non è riuscito a leggere: il disco non ha risposto entro il tempo limite, oppure ha dato errore, oppure i permessi non lo consentivano. GEMINUS li ha marcati come illeggibili e ha continuato col file successivo. Filtra l'albero col chip `Illeggibili` per vedere quali sono.

#### Contatore "Saltati Per Disco Bloccato"

Compare solo nei casi gravi: `(N Saltati Per Disco Bloccato)`. Significa che il disco si è bloccato così tante volte (8 file falliti) che GEMINUS ha smesso di insistere e ha saltato i file rimanenti per non sprecare tempo e risorse.

#### Cosa Fare Quando Vedi Questi Numeri

- **Pochi Illeggibili (1-3)**: probabilmente settori difettosi isolati. Fai un backup urgente dei file ancora leggibili su un altro disco.
- **Molti Illeggibili o Saltati**: il disco è seriamente compromesso. Recupera tutto il salvabile, poi vai alla sezione [Formattare Un Disco](#formattare-un-disco) per riformattare il disco con controllo settori, oppure sostituiscilo se è in fine vita.
- **Disco Con Molti Settori In Attesa Di Rimappatura** (decine o più): considera di riformattarlo col controllo dei settori attivo, così quelli difettosi vengono marcati e il sistema non li usa più. Come si fa, sui due sistemi, è nel capitolo [Formattare Un Disco](#formattare-un-disco).

---

### Formattare Un Disco

> ⚠ **Attenzione**: formattare un disco cancella tutti i dati. Verifica DUE VOLTE quale disco stai formattando prima di procedere. Sbagliare disco significa perdere tutto.

#### Prima Di Tutto — Salvare I Dati Recuperabili

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

##### Passo 1 — Identificare Il Disco

Apri un terminale e dai:

```bash
lsblk -f
```

Riconosci il tuo disco USB dalla dimensione e dall'etichetta. Esempio: una USB da 4TB potrebbe essere `/dev/sdb` con la sua partizione `/dev/sdb1`. **Mai usare `/dev/sda`** senza essere sicurissimi: di solito è il disco di sistema.

##### Passo 2 — Smontare

```bash
sudo umount /dev/sdX1
```

Sostituisci `X` con la lettera reale (es. `sdb1`). Se il disco non era montato, ignora l'errore.

##### Passo 3 — Formattare (Scegli Il Filesystem)

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

##### Passo 4 — Rimontare

Scollega fisicamente la USB e ricollegala: il gestore file la rimonterà in automatico. In alternativa:

```bash
sudo mkdir -p /mnt/miodisco
sudo mount /dev/sdX1 /mnt/miodisco
```

##### Passo 5 — Verifica

```bash
df -h | grep sdX
```

Controlla che lo spazio totale sia quello atteso e che il filesystem sia quello giusto.

---

### Domande Frequenti

**GEMINUS modifica i miei file?**
Non da solo. Il confronto è in sola lettura: guarda e non tocca niente. I file cambiano solo quando lo chiedi tu, e sono due i modi: trascinare qualcosa da una colonna all'altra, o cancellare col tasto destro. Nel primo caso, se c'è un conflitto, GEMINUS chiede sempre cosa fare; nel secondo chiede conferma, e la cancellazione è definitiva.

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

**Perché alcune cartelle sono "Escluse"?**
GEMINUS salta le cartelle che cambiano continuamente da sole, dove il confronto non direbbe niente di utile e costerebbe tempo: `.git`, `node_modules`, `target`, `.cargo`, `.rustup`, `.venv`, `.cache`, `__pycache__`, cestino. *Su Windows* salta anche le cartelle che il sistema tiene per sé (`$RECYCLE.BIN`, `System Volume Information`, `Config.Msi`), quelle che stanno nella radice di ogni unità: non sono tue, e confrontarle non dice niente sul tuo backup. Conta il nome: una cartella tua chiamata così verrebbe saltata comunque. Vale per il confronto, non per la copia: se trascini una cartella che ne contiene una di queste, viene copiata per intero — copiare una cartella deve darti la cartella, non una versione con dei pezzi in meno.

**Quanto è affidabile il confronto Approfondito?**
Molto. GEMINUS legge il contenuto dei due file dall'inizio alla fine: se differiscono anche di un solo byte, lo vede. Il caso in cui due file diversi vengano dichiarati uguali è così improbabile che non capiterà in nessuna vita umana.
