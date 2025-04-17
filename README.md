![image](https://github.com/user-attachments/assets/a70f8ff8-5514-4f6b-b178-871700932123)

## Fmc 
Download the game at [fmc.gg](https://fmc.gg)  

Singleplayer does not work (well) on Windows and MacOS yet, you should download a [standalone server binary](https://github.com/awowogei/FMC_173/releases/tag/nightly).
Click "connect" after running it instead of "singleplayer".
It will be caught by Defender on Windows, and MacOS makes it hard to run unsigned executables.  

Join the [discord](https://discord.gg/VMgFmdsQ6m) if you have questions

## What is this?
Fmc is a platform/[library](https://github.com/formulaicgame/fmc/tree/master/fmc) for playing and
creating [block games](https://github.com/awowogei/FMC_173) that are
[moddable](https://github.com/formulaicgame/fmc/tree/master/examples/server_mod). 
It is designed to make everything moddable and customizable by the server host, from shaders and UI,
to player physics, all playable through the same
[client](https://github.com/formulaicgame/fmc/tree/master/client).

This repository only contains the [client](https://github.com/formulaicgame/fmc/tree/master/client) and
[fmc library](https://github.com/formulaicgame/fmc/tree/master/fmc), you can find [the default
game over here](https://github.com/awowogei/FMC_173)

## Contributing
Contributions are welcome and encouraged, reach out on [discord](https://discord.gg/VMgFmdsQ6m). 

## Modding

Mods can be added through the `fmc build` command, see `fmc build --template` for an example.  
Mods are plain rust crates, and can be found at [crates.io](https://crates.io/search?q=fmc_) by searching for `fmc_`  
To develop your own mod, see the [example mod](examples/server_mod).

## Build client from source
```
git clone https://github.com/formulaicgame/fmc
cd fmc/client && cargo run --release
```

# Licensing
[client](./client/) - All rights reserved (Will be made AGPL3 as the project becomes established)  
[fmc](./fmc/)    - MIT or Apache-2.0
