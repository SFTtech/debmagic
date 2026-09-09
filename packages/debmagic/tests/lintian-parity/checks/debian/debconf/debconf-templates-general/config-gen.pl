#!/usr/bin/perl

# Helper script to generate d/config from d/templates.
# It is just here to make sure all templates are "used".

use strict;
use warnings;

print <<EOF ;
#!/bin/sh

set -e

. /usr/share/debconf/confmodule

EOF

for my $line ( <STDIN> ) {

    if ($line =~ /^Template:\s*(\S+)\s*$/) {

        my $template = $1;
        next
          unless defined $template;

        print "db_input high $template || true\n";
        print "db_go\n\n";
    }
}

exit 0;
