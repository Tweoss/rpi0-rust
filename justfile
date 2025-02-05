default-profile := 'release'

build-run profile=default-profile:
    just build {{profile}}
    just run {{profile}}

build profile=default-profile:
    #!/usr/bin/env bash
    set -euxo pipefail
    path_profile={{ if profile == "dev" { "debug" } else { "release" } }}
    cargo rustc --profile {{profile}} --target armv6zk-none-eabihf.json --package rust -Z build-std="core,compiler_builtins,alloc" -- -C link-arg=-Tlink.x
    arm-none-eabi-objcopy target/armv6zk-none-eabihf/$path_profile/rust -O binary target/armv6zk-none-eabihf/$path_profile/app.bin

run profile=default-profile:
    #!/usr/bin/env bash
    set -euxo pipefail
    path_profile={{ if profile == "dev" { "debug" } else { "release" } }}
    cd installer; cargo run -q ../target/armv6zk-none-eabihf/$path_profile/app.bin

build-copy-boot profile=default-profile:
    just build-boot {{profile}}
    just copy-boot {{profile}}

build-boot profile=default-profile:
    #!/usr/bin/env bash
    set -euxo pipefail
    path_profile={{ if profile == "dev" { "debug" } else { "release" } }}
    cargo rustc --profile {{profile}} --target armv6zk-none-eabihf.json --package bootloader -Z build-std="core,compiler_builtins,alloc" -- -C link-arg=-Tlink.x
    arm-none-eabi-objcopy target/armv6zk-none-eabihf/$path_profile/boot -O binary target/armv6zk-none-eabihf/$path_profile/boot.bin

copy-boot profile=default-profile:
    #!/usr/bin/env bash
    set -euxo pipefail
    path_profile={{ if profile == "dev" { "debug" } else { "release" } }}
    cp target/armv6zk-none-eabihf/$path_profile/boot.bin '/Volumes/No Name/kernel.img'
    sync
    diskutil eject "NO NAME"

profile profile:
    #!/usr/bin/env bash
    set -euxo pipefail
    path_profile={{ if profile == "dev" { "debug" } else { "release" } }}
    # copy paste addr:count pairs into profile-{{profile}}.txt
    arm-none-eabi-objdump -d target/armv6zk-none-eabihf/$path_profile/rust > target/armv6zk-none-eabihf/$path_profile/rust.dump
    python3 lookup.py target/armv6zk-none-eabihf/$path_profile/rust.dump profile-$path_profile.txt
