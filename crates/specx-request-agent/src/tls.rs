//! Read the TLS session used by this transfer, while the header callback still
//! owns a live connection (CURLINFO_TLS_SSL_PTR is not valid after close).
#[derive(Clone, Debug, Default)]
pub struct AgentTls {
    pub protocol: Option<String>,
    pub cipher: Option<String>,
    pub certificate_common_name: Option<String>,
    pub issuer_common_name: Option<String>,
    /// OpenSSL's unambiguous UTC certificate timestamp, including its GMT suffix.
    pub expires_at: Option<String>,
}

// curl-rust does not expose CURLINFO_TLS_SSL_PTR. Keep the native boundary here
// and copy all values before libcurl can release the connection.
#[cfg(unix)]
pub(crate) fn collect(handle: *mut curl_sys::CURL) -> Option<AgentTls> {
    use foreign_types::ForeignTypeRef;
    use openssl::{nid::Nid, ssl::SslRef, x509::X509NameRef};
    use std::{ffi::c_void, os::raw::c_int, ptr};
    #[repr(C)]
    struct TlsSession {
        backend: c_int,
        internals: *mut c_void,
    }
    // Values are stable public libcurl ABI constants since 7.48.0.
    const TLS_SSL_PTR: curl_sys::CURLINFO = curl_sys::CURLINFO_SLIST + 45;
    let mut session: *mut TlsSession = ptr::null_mut();
    // SAFETY: handle is a live Easy handle in its synchronous header callback;
    // getinfo writes a borrowed curl_tlssessioninfo*, never takes ownership.
    let result = unsafe { curl_sys::curl_easy_getinfo(handle, TLS_SSL_PTR, &mut session) };
    if result != curl_sys::CURLE_OK || session.is_null() {
        return None;
    }
    // SAFETY: the returned session belongs to the still-active libcurl handle.
    let session = unsafe { &*session };
    if session.internals.is_null() {
        return None;
    }
    if session.backend == 2 {
        return gnutls::collect(session.internals);
    }
    if session.backend != 1 {
        return None;
    }
    // SAFETY: CURLSSLBACKEND_OPENSSL (1) guarantees internals is SSL*. Both
    // libcurl and openssl use Cargo's single openssl-sys ABI. Never free it.
    let ssl = unsafe { SslRef::from_ptr(session.internals.cast()) };
    let certificate = ssl.peer_certificate();
    let common_name = |name: &X509NameRef| {
        name.entries_by_nid(Nid::COMMONNAME).next().and_then(|entry| entry.data().to_string().ok())
    };
    Some(AgentTls {
        protocol: Some(ssl.version_str().to_owned()),
        cipher: ssl.current_cipher().map(|cipher| cipher.standard_name().unwrap_or(cipher.name()).to_owned()),
        certificate_common_name: certificate.as_ref().and_then(|cert| common_name(cert.subject_name())),
        issuer_common_name: certificate.as_ref().and_then(|cert| common_name(cert.issuer_name())),
        expires_at: certificate.as_ref().map(|cert| cert.not_after().to_string()),
    })
}

// System libcurl may use GnuTLS even though the Rust certificate parser uses
// OpenSSL. Resolve symbols from that already-loaded backend; do not open a
// second connection or require a GnuTLS development package in every host.
#[cfg(unix)]
mod gnutls {
    use super::AgentTls;
    use openssl::{
        nid::Nid,
        x509::{X509, X509NameRef},
    };
    use std::{
        ffi::{CStr, c_char, c_void},
        os::raw::c_uint,
    };
    #[repr(C)]
    struct Datum {
        data: *const u8,
        size: c_uint,
    }
    pub(super) fn collect(session: *mut c_void) -> Option<AgentTls> {
        type Version = unsafe extern "C" fn(*mut c_void) -> c_uint;
        type VersionName = unsafe extern "C" fn(c_uint) -> *const c_char;
        type Cipher = unsafe extern "C" fn(*mut c_void) -> *const c_char;
        type Peers = unsafe extern "C" fn(*mut c_void, *mut c_uint) -> *const Datum;
        macro_rules! symbol {
            ($name:expr, $ty:ty) => {{
                // SAFETY: POSIX dlsym searches the loaded GnuTLS backend. The
                // named public symbols have the exact signatures above.
                let pointer = unsafe { libc::dlsym(libc::RTLD_DEFAULT, $name.as_ptr()) };
                if pointer.is_null() {
                    return None;
                }
                unsafe { std::mem::transmute::<*mut c_void, $ty>(pointer) }
            }};
        }
        let version = symbol!(c"gnutls_protocol_get_version", Version);
        let version_name = symbol!(c"gnutls_protocol_get_name", VersionName);
        let cipher = symbol!(c"gnutls_ciphersuite_get", Cipher);
        let peers = symbol!(c"gnutls_certificate_get_peers", Peers);
        let text = |pointer: *const c_char| {
            // SAFETY: these GnuTLS functions return static NUL-terminated names.
            (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned())
        };
        // SAFETY: libcurl identified backend 2, which exposes gnutls_session_t;
        // it remains live for this synchronous header callback only.
        let protocol = text(unsafe { version_name(version(session)) }).map(|name| name.replacen("TLS", "TLSv", 1));
        let cipher = text(unsafe { cipher(session) });
        let mut count = 0;
        // SAFETY: GnuTLS owns a live array of count peer certificates, leaf first.
        let certificates = unsafe { peers(session, &mut count) };
        let certificate = if count > 0 && !certificates.is_null() {
            let leaf = unsafe { &*certificates };
            if leaf.data.is_null() {
                None
            } else {
                X509::from_der(unsafe { std::slice::from_raw_parts(leaf.data, leaf.size as usize) }).ok()
            }
        } else {
            None
        };
        let common_name = |name: &X509NameRef| {
            name.entries_by_nid(Nid::COMMONNAME).next().and_then(|entry| entry.data().to_string().ok())
        };
        Some(AgentTls {
            protocol,
            cipher,
            certificate_common_name: certificate.as_ref().and_then(|cert| common_name(cert.subject_name())),
            issuer_common_name: certificate.as_ref().and_then(|cert| common_name(cert.issuer_name())),
            expires_at: certificate.as_ref().map(|cert| cert.not_after().to_string()),
        })
    }
}

#[cfg(not(unix))]
pub(crate) fn collect(_handle: *mut curl_sys::CURL) -> Option<AgentTls> {
    None
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use curl::easy::Easy;
    use openssl::{
        asn1::Asn1Time,
        hash::MessageDigest,
        pkey::PKey,
        rsa::Rsa,
        ssl::{SslAcceptor, SslMethod, SslVersion},
        x509::{X509, X509NameBuilder, extension::SubjectAlternativeName},
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };

    #[test]
    fn reads_the_verified_connection_for_tls12_and_tls13_even_when_the_server_closes() {
        for version in [SslVersion::TLS1_2, SslVersion::TLS1_3] {
            let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
            let mut name = X509NameBuilder::new().unwrap();
            name.append_entry_by_text("CN", "localhost").unwrap();
            let name = name.build();
            let mut cert = X509::builder().unwrap();
            cert.set_version(2).unwrap();
            cert.set_subject_name(&name).unwrap();
            cert.set_issuer_name(&name).unwrap();
            cert.set_pubkey(&key).unwrap();
            cert.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
            cert.set_not_after(&Asn1Time::days_from_now(2).unwrap()).unwrap();
            let san = SubjectAlternativeName::new().ip("127.0.0.1").build(&cert.x509v3_context(None, None)).unwrap();
            cert.append_extension(san).unwrap();
            cert.sign(&key, MessageDigest::sha256()).unwrap();
            let cert = cert.build();
            let mut acceptor = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).unwrap();
            acceptor.set_min_proto_version(Some(version)).unwrap();
            acceptor.set_max_proto_version(Some(version)).unwrap();
            acceptor.set_private_key(&key).unwrap();
            acceptor.set_certificate(&cert).unwrap();
            let acceptor = acceptor.build();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut stream = acceptor.accept(socket).unwrap();
                let mut buffer = [0; 4096];
                assert!(stream.read(&mut buffer).unwrap() > 0);
                let protocol = stream.ssl().version_str().to_owned();
                let cipher = stream.ssl().current_cipher().unwrap().standard_name().unwrap().to_owned();
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").unwrap();
                (protocol, cipher)
            });
            let mut easy = Easy::new();
            easy.url(&format!("https://{address}/")).unwrap();
            easy.proxy("").unwrap();
            easy.timeout(Duration::from_secs(5)).unwrap();
            let trust =
                std::env::temp_dir().join(format!("specx-agent-ca-{}-{}.pem", std::process::id(), address.port()));
            std::fs::write(&trust, cert.to_pem().unwrap()).unwrap();
            easy.cainfo(&trust).unwrap();
            let raw = easy.raw();
            let mut tls = None;
            {
                let mut transfer = easy.transfer();
                transfer
                    .header_function(|_| {
                        tls = collect(raw);
                        true
                    })
                    .unwrap();
                transfer.write_function(|data| Ok(data.len())).unwrap();
                transfer.perform().unwrap();
            }
            std::fs::remove_file(trust).unwrap();
            let (protocol, cipher) = server.join().unwrap();
            let tls = tls.expect("verified TLS session must be collected before Connection: close");
            assert_eq!(tls.protocol.as_deref(), Some(protocol.as_str()));
            assert_eq!(tls.cipher.as_deref(), Some(cipher.as_str()));
            assert_eq!(tls.certificate_common_name.as_deref(), Some("localhost"));
            assert_eq!(tls.issuer_common_name.as_deref(), Some("localhost"));
            assert_eq!(tls.expires_at.as_deref(), Some(cert.not_after().to_string().as_str()));
        }
    }
}
