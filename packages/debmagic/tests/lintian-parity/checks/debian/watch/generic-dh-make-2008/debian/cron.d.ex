#
# Regular cron jobs for the generic-dh-make-2008 package
#
0 4	* * *	root	[ -x /usr/bin/generic-dh-make-2008_maintenance ] && /usr/bin/generic-dh-make-2008_maintenance
