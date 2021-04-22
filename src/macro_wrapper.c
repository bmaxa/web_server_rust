#include <openssl/ssl.h>

void rust_SSL_library_init(){
    SSL_library_init();
}

void rust_OpenSSL_add_all_algorithms(){
    OpenSSL_add_all_algorithms();
}

void rust_SSL_load_error_strings(){
    SSL_load_error_strings();
}

const SSL_METHOD* rust_SSLv23_method(){
    return SSLv23_method();
}
