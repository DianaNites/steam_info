# Steam Info

Get the minimum steam system info for ProtonDB

Primarily intended to allow easily getting system info on dual GPU systems,
without having to restart Steam on your dedicated GPU.

As a side effect, it includes far less information about your system than
the standard Steam system information.

## Usage

```shell
$ DRI_PRIME=1 steam_info
System Info:


Processor Information:
    CPU Brand:  AMD Ryzen 9 5980HX with Radeon Graphics

Operating System Version:
    "Arch Linux" (64 bit)
    Kernel Name:  Linux
    Kernel Version:  6.0.6-arch1-1

Video Card:
    Driver:  AMD AMD Radeon RX 6800M (navi22, LLVM 14.0.6, DRM 3.48, 6.0.6-arch1-1)
    Driver Version:  4.6 (Compatibility Profile) Mesa 22.2.1



Memory:
    RAM:  63710 MB
```
