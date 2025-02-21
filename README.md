![image](https://github.com/user-attachments/assets/f72ae725-2492-4677-8913-c12d23a5fd27)

## Fmc 
Download the game at [fmc.gg](https://fmc.gg)  
Join the [Discord](https://discord.gg/VMgFmdsQ6m) if you have questions

## Modding

Mods can be added through the `fmc build` command, see `fmc build --template` for an example.  
Mods are plain rust crates, and can be found at [crates.io](https://crates.io/search?q=fmc_) by searching for `fmc_`  
To develop your own mod, see the [example mod](examples/mod).

## Build client from source
```
git clone https://github.com/formulaicgame/fmc
cd fmc/client && cargo run --release
```
# Licensing
[client](./client/) - All rights reserved (Will be made AGPL3 with time)  
[fmc](./fmc/)    - MIT or Apache-2.0
