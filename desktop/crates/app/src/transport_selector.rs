//! Picks the active transport for a given UDID when both Wi-Fi and USB are
//! visible at the same time. Per ADR-004 + ADR-010, the cable wins.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    Wifi,
    Usb,
}

impl Candidate {
    pub fn wifi() -> Self {
        Self::Wifi
    }
    pub fn usb() -> Self {
        Self::Usb
    }
}

#[derive(Default)]
pub struct TransportSelector;

impl TransportSelector {
    pub fn new() -> Self {
        Self
    }

    pub fn decide_active(&self, _udid: &str, candidates: &[Candidate]) -> Option<Candidate> {
        if candidates.contains(&Candidate::Usb) {
            Some(Candidate::Usb)
        } else if candidates.contains(&Candidate::Wifi) {
            Some(Candidate::Wifi)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_wins_against_wifi_for_same_udid() {
        let s = TransportSelector::new();
        assert_eq!(
            s.decide_active("UDID-1", &[Candidate::wifi(), Candidate::usb()]),
            Some(Candidate::usb())
        );
    }

    #[test]
    fn lone_wifi_is_kept() {
        let s = TransportSelector::new();
        assert_eq!(
            s.decide_active("UDID-1", &[Candidate::wifi()]),
            Some(Candidate::wifi())
        );
    }

    #[test]
    fn no_candidates_means_no_active() {
        let s = TransportSelector::new();
        assert_eq!(s.decide_active("UDID-1", &[]), None);
    }
}
