# tool14

This is a project that extracts scripts for FFXIV's quests and formats them to show dialogue.

## Requirements

- Git
- Rust 1.99
- A local installation of FFXIV

## Building from Source

To clone the repository:
```
git clone https://github.com/Silenthal/tool14
cd tool14
```

To build:
```
cargo build --release 
```
The program binary will be output to `/target/release/`.

## Usage

To run from the repository:

```
cargo run -- [OPTIONS]
```

Or the binary:

```
tool14 [OPTIONS]
```

Program options:
```
  -u, --force-update
          True to force a database update
  -a, --read-all
          True to read all quests
  -s, --detail-level <DETAIL_LEVEL>
          The detail level of the file. 1 = dialogue lines only 2 = dialogue lines with some Lua script 3 = dialogue lines with all Lua script lines
  -o, --output <OUTPUT>
          The name of the file to write the dialogue to
  -i, --quest-number <QUEST_NUMBER>
          The 5-digit quest number
  -q, --quest-id <QUEST_ID>
          All or part of the quest ID
  -n, --quest-name <QUEST_NAME>
          All or part of the name of the quest
  -p, --ffxiv-install-path <FFXIV_INSTALL_PATH>
          The path to the FFXIV installation folder. Saves to config after program exits
  -f, --quest-id-file <QUEST_ID_FILE>
          A file containing a list of quest IDs or numbers, one per line
  -h, --help
          Print help
```

## Sources

- ackwell/ironworks (https://github.com/ackwell/ironworks)
- WorkingRobot/ironworks https://github.com/WorkingRobot/ironworks
- XIV Data Oxidizer (https://github.com/fae-industries/xiv-data-oxidizer)
- EXDSchema (https://github.com/xivdev/EXDSchema)
- XIViewer (https://github.com/WorkingRobot/XIViewer)
