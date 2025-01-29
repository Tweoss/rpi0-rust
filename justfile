build-run:
    just build
    just run

build:
    cargo build
    # make sure that the file starts with _start pretty please 
    arm-none-eabi-objdump -d target/armv6zk-none-eabihf/debug/rust | grep "00008000 <_start>:" 
    arm-none-eabi-objcopy target/armv6zk-none-eabihf/debug/rust -O binary target/armv6zk-none-eabihf/debug/boot.bin

run:
    pi-install target/armv6zk-none-eabihf/debug/boot.bin

build-release:
    cargo build --release
    arm-none-eabi-objdump -d target/armv6zk-none-eabihf/release/rust | grep "00008000 <_start>:" 
    arm-none-eabi-objcopy target/armv6zk-none-eabihf/release/rust -O binary target/armv6zk-none-eabihf/release/boot.bin
run-release:
    pi-install target/armv6zk-none-eabihf/release/boot.bin


profile:
    # copy paste addr:count pairs into profile.txt
    # 
    # arm-none-eabi-objdump -d target/armv6zk-none-eabihf/release/rust > target/armv6zk-none-eabihf/release/rust.dump
    # python3 lookup.py target/armv6zk-none-eabihf/release/rust.dump profile.txt
    arm-none-eabi-objdump -d target/armv6zk-none-eabihf/debug/rust > target/armv6zk-none-eabihf/debug/rust.dump
    python3 lookup.py target/armv6zk-none-eabihf/debug/rust.dump profile.txt
