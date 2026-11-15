# ADR 0016: HTTP client and TLS

- Status: Accepted
- Date: 2026-11-15

## Decision

`craftward-core` defines the HTTP call it needs and does not depend on a
network crate. `craftward-net` is the only crate that speaks HTTP. The
command-line tool depends on it. The desktop application does not, until it
grows a radar view.

The client is ureq 3.5.2 with default features off and `rustls-no-provider`
on. TLS is rustls 0.23.46 with default features off and `ring`, `std`, and
`tls12` on. The cryptography provider is ring. Root certificates come from
the operating system through rustls-native-certs 0.8.5, passed to ureq as
an explicit certificate list. If that list is empty, the client fails. It
does not fall back to a bundled Mozilla list.

Connections are HTTPS only, including redirects. The connect timeout is 10
seconds and the send and receive timeouts are 30 seconds. A response body
is limited to 10 MB. The user agent is `craftward/<version>`.

reqwest was not used. A hand-written HTTP implementation was not used. The
ureq `rustls` feature was not used, because it brings webpki-roots
(CDLA-Permissive-2.0), which this repository does not accept. The ureq
`platform-verifier` feature was not used for the same license on
webpki-root-certs, and because that tree also adds more packages than this
package allows. aws-lc-rs was not used because its Windows build expects
NASM.

## Rationale

The core stays testable without a network, and only the programs that scan
live data take the HTTP dependency. ureq is a small client with a rustls
feature that does not choose a provider, so the provider and the
certificate source can be selected here. ring is the provider ureq
documents as the one most likely to build without extra tools, and it
builds from the crates.io package on Windows without NASM or CMake.
rustls-native-certs is a direct dependency because both root lists bundled
through ureq use a license this repository rejects, while the operating
system store does not add that license.
