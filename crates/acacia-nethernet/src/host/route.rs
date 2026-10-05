use crate::turn::stun::is_stun;

const HEADER: usize = 20;
const BINDING_REQUEST: [u8; 2] = [0x00, 0x01];
const USERNAME: u16 = 0x0006;

/// The local half of the USERNAME (`<ours>:<theirs>`) in an ICE binding request; `None` for any
/// other datagram.
pub(super) fn request_ufrag(data: &[u8]) -> Option<&str> {
    if !is_stun(data) || data[..2] != BINDING_REQUEST {
        return None;
    }
    let mut attrs = data.get(HEADER..)?;
    while let [t0, t1, l0, l1, rest @ ..] = attrs {
        let len = usize::from(u16::from_be_bytes([*l0, *l1]));
        let value = rest.get(..len)?;
        if u16::from_be_bytes([*t0, *t1]) == USERNAME {
            return std::str::from_utf8(value).ok()?.split(':').next();
        }
        // Values are padded to four bytes.
        attrs = rest.get(len.next_multiple_of(4)..)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::stun::{Attr, Class, Message, Method};

    #[test]
    fn finds_the_local_ufrag_of_binding_requests_only() {
        let request = Message::new(Class::Request, Method::Binding, [7; 12])
            .with(Attr::Software("x".into()))
            .with(Attr::Username("Ab12:Zz99".into()))
            .encode(None, true);
        assert_eq!(request_ufrag(&request), Some("Ab12"));
        let response = Message::new(Class::Success, Method::Binding, [7; 12]).with(Attr::Username("Ab12:Zz99".into())).encode(None, false);
        assert_eq!(request_ufrag(&response), None);
        // Binding-request type bytes without the magic cookie: not STUN.
        assert_eq!(request_ufrag(&[0x00, 0x01, 0, 8, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 4]), None);
        assert_eq!(request_ufrag(&request[..request.len() - 30]), None);
    }
}
