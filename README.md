# Fmc 
[![Discord](https://img.shields.io/discord/1289906834504810528.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/VMgFmdsQ6m)

![image](https://github.com/user-attachments/assets/f72ae725-2492-4677-8913-c12d23a5fd27)

## How to run
```
cd client && cargo run --release
```

Singleplayer takes a little while to work as it must download the server(10mb).

## Modding

Mods can be added through the `fmc build` command, see also `fmc build --template` for an example.
Mods are rust crates, and can be found at [crates.io](https://crates.io/search?q=fmc_) by searching for `fmc_`  
To develop your own mod, see the [example mod](examples/mod)

# Licensing
[client](./client/) - AGPLv3  
[fmc](./fmc/)    - MIT or Apache-2.0
