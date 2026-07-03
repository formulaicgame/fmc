![image](https://github.com/user-attachments/assets/a70f8ff8-5514-4f6b-b178-871700932123)

## Installation 

Download the game at [fmc.gg](https://fmc.gg)  

### Windows

I'm unsure if singleplayer works, you might need to download a
[standalone server binary](https://github.com/awowogei/FMC_173/releases/tag/nightly) 
and connect through multiplayer.  
@ me if it doesn't work, thanks.

### MacOS

Install, then do to the this to bypass Gatekeeper:  
```
open the app(it will fail), then go to System Settings > Privacy & Security > scroll to bottom > open anyway
```
Or if you're comfortable in the terminal:  
```shell
xattr -d com.apple.quarantine /Applications/fmc.app
```

Ask on [discord](https://discord.gg/VMgFmdsQ6m) if you have questions/problems

## What is this?

Fmc lets you create and play block games that are [moddable](examples/server_mod).  
The aim is to enable modification of any aspect of the game entirely server-side, so that you can play
different games through the same client.  

This repository contains the 
[client](https://github.com/formulaicgame/fmc/tree/master/client)
and a
[library](https://github.com/formulaicgame/fmc/tree/master/fmc)
that can be used to implement games, you can find
[the current default game here](https://github.com/awowogei/FMC_173).

## Contributing

Contributions are welcome, please reach out on [discord](https://discord.gg/VMgFmdsQ6m).

## Modding
> [!note]
> Keep in mind it downloads a local version of the rust compiler when you run `fmc build`, it does
> not use your system version.

Server mods can be added through the `fmc build` command, see `fmc build --template` for an example.  
Mods are plain rust crates, and can be found at [crates.io](https://crates.io/search?q=fmc_) by searching for `fmc_`  
To develop your own mod, see the [example mod](examples/server_mod).

**Windows users**: Follow these 
[instructions](https://rust-lang.github.io/rustup/installation/windows-msvc.html#installing-only-the-required-components-optional)
if you don't already have rust installed. If you are uncomfortable with this you should avoid modding until a way can be found to do it
automatically.

## Build client from source

```
git clone https://github.com/formulaicgame/fmc
cd fmc/client && cargo run --release
```

## Licensing

[client](./client/) - AGPL-3.0  
[fmc](./fmc/)    - MIT or Apache-2.0
