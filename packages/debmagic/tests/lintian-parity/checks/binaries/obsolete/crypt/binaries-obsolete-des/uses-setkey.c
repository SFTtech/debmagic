/* This program uses the obsolete function 'setkey', which sets a key for
   DES encryption.  */

#define _GNU_SOURCE 1
#include <stdlib.h>
#include <errno.h>
#include <stdio.h>

/* The prototype of 'setkey' may already have been removed from
   stdlib.h.  */
extern void setkey(const char *);


/* It may already not be possible to link new programs that use
   'setkey' without special magic.  */
#ifdef SYMVER
__asm__ (".symver setkey, setkey@" SYMVER);
#endif

/* setkey uses a 1-bit-per-byte representation of a DES key.
   Yes, really.  */
const char key[64] = {
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
};

int
main(void)
{
    /* The primary effects of calling 'setkey' are only visible by
       calling 'encrypt', and we don't want to call 'encrypt' in this
       program because we want to make sure Lintian detects programs
       that call 'setkey' but not 'encrypt', even though that doesn't
       make a whole lot of sense.  So we just call it and then check
       whether it changed errno, which is the documented way to check
       whether it failed.  */
    errno = 0;
    setkey(key);
    if (errno) {
        perror("setkey");
    }
    return 0;
}
