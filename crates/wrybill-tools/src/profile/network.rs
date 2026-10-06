//! Whether this computer has a way out to the internet.
//!
//! Wrybill asks the OS and sends nothing (spec 10.3). It asks for a route to
//! an address, by connecting a UDP socket to it. Connecting a UDP socket only
//! records where it would send to, so no packet leaves the computer.
//!
//! The addresses are the ones reserved for documentation (RFC 5737 and
//! RFC 3849). They belong to nobody, so no company is being "checked".

use std::net::UdpSocket;

/// An IPv4 address reserved for documentation, with the discard port.
const IPV4_ELSEWHERE: &str = "192.0.2.1:9";

/// An IPv6 address reserved for documentation, with the discard port.
const IPV6_ELSEWHERE: &str = "[2001:db8::1]:9";

/// What the OS says about the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// The OS has a route out. Nothing was sent, so this doesn't prove that
    /// the internet answers.
    Up,
    /// The OS has no route out.
    NoRoute,
}

/// Asks the OS whether it has a route out, over IPv4 or IPv6. Either one
/// counts, and any failure counts as no route.
pub(super) fn check() -> Network {
    if has_route("0.0.0.0:0", IPV4_ELSEWHERE) || has_route("[::]:0", IPV6_ELSEWHERE) {
        Network::Up
    } else {
        Network::NoRoute
    }
}

/// Whether the OS can pick a route from `local` to `remote`.
fn has_route(local: &str, remote: &str) -> bool {
    UdpSocket::bind(local)
        .and_then(|socket| socket.connect(remote))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::has_route;

    #[test]
    fn a_computer_always_has_a_route_to_itself() {
        assert!(has_route("127.0.0.1:0", "127.0.0.1:9"));
    }

    #[test]
    fn an_address_that_makes_no_sense_counts_as_no_route() {
        assert!(!has_route("0.0.0.0:0", "not an address"));
        assert!(!has_route("not an address", "192.0.2.1:9"));
    }
}
