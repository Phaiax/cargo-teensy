Rust @ Teensy 3.1
=================

```
    cd my_code_dir
    rustup override set nightly-2016-05-24 # see *
    cargo install cargo-teensy
    cargo teensy new my_project
    cd my_project
    cargo teensy upload
    # press the reset button on the teensy
```

\* `nightly-2016-05-24` or the version mentioned [here](https://github.com/hackndev/zinc) .

Needed software:
 * rustup
 * [teensy_loader_cli](https://www.pjrc.com/teensy/loader_cli.html) in $PATH
 * FEDORA24: Packages:
    * openssl-devel
    * arm-none-eabi-newlib-2.2.0_1-7.fc24.noarch
    * arm-none-eabi-binutils-cs-1:2.25-3.fc24.x86_64
    * arm-none-eabi-gcc-cs-c++-1:5.2.0-4.fc24.x86_64
    * arm-none-eabi-gcc-cs-1:5.2.0-4.fc24.x86_64
 * UBUNTU
    * gcc-arm-none-eabi
    * more?? Please file issue