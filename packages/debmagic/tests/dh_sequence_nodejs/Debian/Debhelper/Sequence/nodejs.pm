#!/usr/bin/perl
use strict;
use warnings;
use Debian::Debhelper::Dh_Lib;

insert_after("dh_install", "dh_nodejs");

1;
