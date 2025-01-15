build-run:
    cargo build
    # make sure that the file starts with _start pretty please 
    arm-none-eabi-objdump -d target/armv6zk-none-eabihf/debug/rust | grep "00008000 <_start>:" 
    arm-none-eabi-objcopy target/armv6zk-none-eabihf/debug/rust -O binary target/armv6zk-none-eabihf/debug/boot.bin
    pi-install target/armv6zk-none-eabihf/debug/boot.bin
