![image](https://github.com/user-attachments/assets/a70f8ff8-5514-4f6b-b178-871700932123)

## Fmc 
Download the game at [fmc.gg](https://fmc.gg)  

Singleplayer does not work (well) on Windows and MacOS yet, you might want to download a [standalone server binary](https://github.com/awowogei/FMC_173/releases/tag/nightly) 
and connect through multiplayer.
It will be caught by Defender on Windows, and MacOS makes it hard to run unsigned executables.  

Join the [discord](https://discord.gg/VMgFmdsQ6m) if you have questions

## What is this?
Fmc lets you create and play block games that are
[moddable](examples/server_mod). 
It aims to enable modification of any aspect of the game, entirely server-side, letting you play a
variety of block games through the same client.

This repository contains the [client](https://github.com/formulaicgame/fmc/tree/master/client) and
a [library](https://github.com/formulaicgame/fmc/tree/master/fmc) that can be used to implement
games, you can find [the current default game here](https://github.com/awowogei/FMC_173).

## Contributing
Contributions are welcome and encouraged, reach out on [discord](https://discord.gg/VMgFmdsQ6m). 

## Modding

Mods can be added through the `fmc build` command, see `fmc build --template` for an example.  
Mods are plain rust crates, and can be found at [crates.io](https://crates.io/search?q=fmc_) by searching for `fmc_`  
To develop your own mod, see the [example mod](examples/server_mod).

**Windows users**: Follow these [instructions](https://rust-lang.github.io/rustup/installation/windows-msvc.html#installing-only-the-required-components-optional)
if you don't already have rust installed. If you are uncomfortable with this you should avoid modding until we figure out a way to do it
automatically.

## Build client from source
```
git clone https://github.com/formulaicgame/fmc
cd fmc/client && cargo run --release
```

# Licensing
[client](./client/) - All rights reserved (Will be made AGPL3 once the project is established)  
[fmc](./fmc/)    - MIT or Apache-2.0
