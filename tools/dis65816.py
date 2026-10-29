"""Tiny 65816 disassembler for reading SMW code (LoROM, headerless or headered ROM).

python tools/dis65816.py <rom> <snes address hex> [count] [m16] [x16]
Tracks REP/SEP to size immediates.
"""
import sys

N = ['BRK','ORA','COP','ORA','TSB','ORA','ASL','ORA','PHP','ORA','ASL','PHD','TSB','ORA','ASL','ORA',
     'BPL','ORA','ORA','ORA','TRB','ORA','ASL','ORA','CLC','ORA','INC','TCS','TRB','ORA','ASL','ORA',
     'JSR','AND','JSL','AND','BIT','AND','ROL','AND','PLP','AND','ROL','PLD','BIT','AND','ROL','AND',
     'BMI','AND','AND','AND','BIT','AND','ROL','AND','SEC','AND','DEC','TSC','BIT','AND','ROL','AND',
     'RTI','EOR','WDM','EOR','MVP','EOR','LSR','EOR','PHA','EOR','LSR','PHK','JMP','EOR','LSR','EOR',
     'BVC','EOR','EOR','EOR','MVN','EOR','LSR','EOR','CLI','EOR','PHY','TCD','JML','EOR','LSR','EOR',
     'RTS','ADC','PER','ADC','STZ','ADC','ROR','ADC','PLA','ADC','ROR','RTL','JMP','ADC','ROR','ADC',
     'BVS','ADC','ADC','ADC','STZ','ADC','ROR','ADC','SEI','ADC','PLY','TDC','JMP','ADC','ROR','ADC',
     'BRA','STA','BRL','STA','STY','STA','STX','STA','DEY','BIT','TXA','PHB','STY','STA','STX','STA',
     'BCC','STA','STA','STA','STY','STA','STX','STA','TYA','STA','TXS','TXY','STZ','STA','STZ','STA',
     'LDY','LDA','LDX','LDA','LDY','LDA','LDX','LDA','TAY','LDA','TAX','PLB','LDY','LDA','LDX','LDA',
     'BCS','LDA','LDA','LDA','LDY','LDA','LDX','LDA','CLV','LDA','TSX','TYX','LDY','LDA','LDX','LDA',
     'CPY','CMP','REP','CMP','CPY','CMP','DEC','CMP','INY','CMP','DEX','WAI','CPY','CMP','DEC','CMP',
     'BNE','CMP','CMP','CMP','PEI','CMP','DEC','CMP','CLD','CMP','PHX','STP','JML','CMP','DEC','CMP',
     'CPX','SBC','SEP','SBC','CPX','SBC','INC','SBC','INX','SBC','NOP','XBA','CPX','SBC','INC','SBC',
     'BEQ','SBC','SBC','SBC','PEA','SBC','INC','SBC','SED','SBC','PLX','XCE','JSR','SBC','INC','SBC']

M = {}
for row in range(16):
    b = row << 4
    if row % 2 == 0:
        M.update({b+1: '(dp,X)', b+3: 'sr,S', b+5: 'dp', b+7: '[dp]', b+9: '#m', b+0xD: 'abs', b+0xF: 'long'})
    else:
        M.update({b+1: '(dp),Y', b+2: '(dp)', b+3: '(sr,S),Y', b+5: 'dp,X', b+7: '[dp],Y', b+9: 'abs,Y', b+0xD: 'abs,X', b+0xF: 'long,X'})
for o in (0x08,0x0A,0x0B,0x18,0x1A,0x1B,0x28,0x2A,0x2B,0x38,0x3A,0x3B,0x40,0x48,0x4A,0x4B,0x58,0x5A,0x5B,0x60,0x68,0x6A,0x6B,0x78,0x7A,0x7B,
          0x88,0x8A,0x8B,0x98,0x9A,0x9B,0xA8,0xAA,0xAB,0xB8,0xBA,0xBB,0xC8,0xCA,0xCB,0xD8,0xDA,0xDB,0xE8,0xEA,0xEB,0xF8,0xFA,0xFB):
    M[o] = 'imp'
for o in (0x10,0x30,0x50,0x70,0x90,0xB0,0xD0,0xF0,0x80):
    M[o] = 'rel'
M.update({0x00:'#8',0x02:'#8',0x42:'#8',0x82:'rel16',0x62:'rel16',0x20:'abs',0x22:'long',0xFC:'(abs,X)',0x7C:'(abs,X)',0x4C:'abs',0x5C:'long',0x6C:'(abs)',0xDC:'[abs]',
  0x04:'dp',0x0C:'abs',0x14:'dp',0x1C:'abs',0x06:'dp',0x0E:'abs',0x16:'dp,X',0x1E:'abs,X',0x24:'dp',0x2C:'abs',0x34:'dp,X',0x3C:'abs,X',0x26:'dp',0x2E:'abs',0x36:'dp,X',0x3E:'abs,X',
  0x44:'mv',0x54:'mv',0x46:'dp',0x4E:'abs',0x56:'dp,X',0x5E:'abs,X',0x64:'dp',0x74:'dp,X',0x9C:'abs',0x9E:'abs,X',0x66:'dp',0x6E:'abs',0x76:'dp,X',0x7E:'abs,X',
  0x84:'dp',0x86:'dp',0x8C:'abs',0x8E:'abs',0x94:'dp,X',0x96:'dp,Y',0x89:'#m',
  0xA0:'#x',0xA2:'#x',0xA4:'dp',0xA6:'dp',0xAC:'abs',0xAE:'abs',0xB4:'dp,X',0xB6:'dp,Y',0xBC:'abs,X',0xBE:'abs,Y',
  0xC0:'#x',0xC2:'#8',0xC4:'dp',0xC6:'dp',0xCC:'abs',0xCE:'abs',0xD4:'dp',0xD6:'dp,X',0xDE:'abs,X',
  0xE0:'#x',0xE2:'#8',0xE4:'dp',0xE6:'dp',0xEC:'abs',0xEE:'abs',0xF4:'abs',0xF6:'dp,X',0xFE:'abs,X'})
SIZE = {'imp':0,'#8':1,'dp':1,'dp,X':1,'dp,Y':1,'(dp)':1,'(dp,X)':1,'(dp),Y':1,'[dp]':1,'[dp],Y':1,'sr,S':1,'(sr,S),Y':1,'rel':1,
        'abs':2,'abs,X':2,'abs,Y':2,'(abs)':2,'(abs,X)':2,'[abs]':2,'rel16':2,'mv':2,'long':3,'long,X':3}


def off(a):
    return ((a >> 16) & 0x7F) * 0x8000 + (a & 0x7FFF)


def dis(rom, a, count, m8=True, x8=True):
    for _ in range(count):
        o = off(a)
        op = rom[o]
        mode = M.get(op, 'imp')
        if mode == '#m':
            n = 1 if m8 else 2
        elif mode == '#x':
            n = 1 if x8 else 2
        else:
            n = SIZE[mode]
        arg = int.from_bytes(rom[o + 1:o + 1 + n], 'little')
        if mode == 'rel':
            s = f'${(a + 2 + (arg - 256 if arg > 127 else arg)) & 0xFFFF | a & 0xFF0000:06X}'
        elif mode == 'rel16':
            s = f'${(a + 3 + (arg - 65536 if arg > 32767 else arg)) & 0xFFFF | a & 0xFF0000:06X}'
        elif mode.startswith('#'):
            s = f'#${arg:0{n * 2}X}'
        elif mode == 'imp':
            s = ''
        else:
            s = mode.replace('long', f'${arg:06X}').replace('abs', f'${arg:04X}').replace('dp', f'${arg:02X}').replace('sr', f'${arg:02X}')
        if op == 0xC2:
            m8, x8 = m8 and not arg & 0x20, x8 and not arg & 0x10
        if op == 0xE2:
            m8, x8 = m8 or bool(arg & 0x20), x8 or bool(arg & 0x10)
        print(f'{a:06X}  {rom[o:o + 1 + n].hex(" "):12} {N[op]} {s}')
        a += 1 + n


if __name__ == '__main__':
    rom = open(sys.argv[1], 'rb').read()
    if len(rom) % 1024:
        rom = rom[512:]
    a = int(sys.argv[2], 16)
    count = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    dis(rom, a, count, 'm16' not in sys.argv, 'x16' not in sys.argv)
