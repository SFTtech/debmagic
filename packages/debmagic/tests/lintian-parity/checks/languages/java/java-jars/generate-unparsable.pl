#!/usr/bin/perl

use strict;
use warnings;

# Generated with "hexdump -C <valid-file.zip> | head -n 10".  Should
# be a valid header of a truncated zip file.  This is enough to fool
# file 5.30 and earlier, but will obviously break if you try to parse
# it in full.
my $valid_header = <<'EOF';
00000000  50 4b 03 04 0a 00 00 00  00 00 4c 59 9e 4a 00 00  |PK........LY.J..|
00000010  00 00 00 00 00 00 00 00  00 00 04 00 1c 00 6f 72  |..............or|
00000020  67 2f 55 54 09 00 03 20  c6 05 59 20 c6 05 59 75  |g/UT... ..Y ..Yu|
00000030  78 0b 00 01 04 e8 03 00  00 04 e8 03 00 00 50 4b  |x.............PK|
00000040  03 04 0a 00 00 00 00 00  4c 59 9e 4a 00 00 00 00  |........LY.J....|
00000050  00 00 00 00 00 00 00 00  0b 00 1c 00 6f 72 67 2f  |............org/|
00000060  64 65 62 69 61 6e 2f 55  54 09 00 03 20 c6 05 59  |debian/UT... ..Y|
00000070  20 c6 05 59 75 78 0b 00  01 04 e8 03 00 00 04 e8  | ..Yux..........|
00000080  03 00 00 50 4b 03 04 0a  00 00 00 00 00 4c 59 9e  |...PK........LY.|
00000090  4a 00 00 00 00 00 00 00  00 00 00 00 00 13 00 1c  |J...............|
EOF

open(my $fd, '>', 'unparsable.jar');

for my $line (split(m/\n/, $valid_header)) {
    chomp($line);
    next if $line =~ s/^\s*+(?:\#.*)?$//;
    next if $line !~ s/^[0-9a-fA-F]+\s+//; # Remove leading "offset"
    $line =~ s/\s*(?:\|.+\|\s*)?$//; # Remove trailing "display" part (if present)
    for my $byte (split(m/\s++/, $line)) {
        printf {$fd} '%c', hex($byte);
    }
}

close($fd);

