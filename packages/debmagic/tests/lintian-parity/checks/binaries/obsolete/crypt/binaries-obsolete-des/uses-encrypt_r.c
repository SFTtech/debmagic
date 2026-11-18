/* This program uses the obsolete function 'encrypt_r', which performs
   DES encryption.  */

#define _GNU_SOURCE 1
#include <crypt.h>
#include <string.h>
#include <stdio.h>

/* The prototype of 'encrypt_r' may already have been removed from
   crypt.h.  */
extern void encrypt_r(char block[64], int edflag, struct crypt_data *data);

/* It may already not be possible to link new programs that use
   'encrypt_r' without special magic.  */
#ifdef SYMVER
__asm__ (".symver encrypt_r, encrypt_r@" SYMVER);
#endif

int
main(void)
{
    struct crypt_data data;
    char block[64];

    memset(&data, 0, sizeof data);
    memset(block, 0, sizeof block);
    encrypt_r(block, 0, &data);
    for (size_t i = 0; i < sizeof block; i++) {
        putchar(block[i] ? '1' : '0');
    }
    putchar('\n');
    return 0;
}
