import sys

def main():
    map = dict()
    current = [0, ""]
    f = open(sys.argv[1], "r")
    for line in f:
        if line.startswith("000"):
            addr, name = line.split(' ')
            current = [int(addr, 16), name.strip(":\n")]
        elif line.startswith("   "):
            addr, _, instr = line.split(maxsplit=2)
            map[int(addr.strip(":"), 16)] = [current, instr]

    
    f.close()
    f = open(sys.argv[2], "r")
    line = f.readline()
    pairs = [pair.split(":") for pair in line.split(',')]
    mapped = [[int(count), addr, map[int(addr.lstrip("0x"), 16)]] for addr, count in pairs]
    mapped.sort()
    for [count, addr, [[_, function], instr]] in mapped:
        print(f"{count}\t{addr} = {instr.strip()}\t ({function})")

main()
