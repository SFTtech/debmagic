/* This program uses the obsolete function 'fcrypt',
   which is an alias for 'crypt'.  */

#include <crypt.h>
#include <stdio.h>

/* The prototype may already have been removed from crypt.h.  */
extern char *fcrypt(const char *, const char *);

/* It may already not be possible to link new programs that use
   'fcrypt' without special magic.  */
#ifdef SYMVER
__asm__ (".symver fcrypt, fcrypt@" SYMVER);
#endif

int
main(void)
{
    puts(fcrypt("password", "Dn"));
    return 0;
}
