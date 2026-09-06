/* This program uses the obsolete function 'setkey_r', which sets a key for
   DES encryption.  */

#define _GNU_SOURCE 1
#include <crypt.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>

/* The prototype of 'setkey_r' may already have been removed from
   crypt.h.  */
extern void setkey_r(const char *, struct crypt_data *);

/* It may already not be possible to link new programs that use
   'setkey_r' without special magic.  */
#ifdef SYMVER
__asm__ (".symver setkey_r, setkey_r@" SYMVER);
#endif

/* setkey_r uses a 1-bit-per-byte representation of a DES key.
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
    struct crypt_data data;
    memset(&data, 0, sizeof data);

    /* The primary effects of calling 'setkey_r' are only visible by
       calling 'encrypt_r', and we don't want to call 'encrypt_r' in
       this program because we want to make sure Lintian detects
       programs that call 'setkey_r' but not 'encrypt_r', even though
       that doesn't make a whole lot of sense.  So we just call it and
       then check whether it changed errno, which is the documented
       way to check whether it failed.  */
    errno = 0;
    setkey_r(key, &data);
    if (errno) {
        perror("setkey_r");
    }
    return 0;
}
