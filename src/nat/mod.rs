//! NAT core implementation

pub mod checksum;
pub mod entry;
pub mod table;

pub use entry::{NatEntry, Protocol};
pub use table::NatTable;

use crate::ffi::*;
use crate::packet::PacketContext;

/// Process outbound packet (LAN -> WAN)
#[no_mangle]
pub extern "C" fn rust_nat_outbound(pkt: *mut NetPkt) -> i32 {
    //log::info!("➡️  Outbound NAT işlemi başlatıldı.");
    if pkt.is_null() {
        return -1;
    }

    let table = match crate::get_nat_table() {
        Some(t) => t,
        None => {
            log::error!("get_nat_table-out işlemi başarısız.");
            return -1;
        }
    };

    // Parse packet
    let mut ctx = match PacketContext::from_pkt(pkt) {
        Some(c) => c,
        None => return -1,
    };

    // Perform NAT translation
    match table.translate_outbound(&mut ctx) {
        Ok(_) => {
            // Apply changes back to packet
            ctx.apply_to_pkt(pkt);
            0
        }
        Err(_) => -1,
    }
}

/// Process inbound packet (WAN -> LAN)
#[no_mangle]
pub extern "C" fn rust_nat_inbound(pkt: *mut NetPkt) -> i32 {
    //log::info!("⬅️  Inbound NAT işlemi başlatıldı.");
    if pkt.is_null() {
        return -1;
    }

    let table = match crate::get_nat_table() {
        Some(t) => t,
        None => {
            log::error!("get_nat_table-in işlemi başarısız.");
            return -1;
        }
    };

    // Parse packet
    let mut ctx = match PacketContext::from_pkt(pkt) {
        Some(c) => c,
        None => return -1,
    };

    // Perform NAT translation
    match table.translate_inbound(&mut ctx) {
        Ok(_) => {
            // Apply changes back to packet
            ctx.apply_to_pkt(pkt);
            0
        }
        Err(_) => -1,
    }
}

/// Configure NAT (called from C)
#[no_mangle]
pub extern "C" fn rust_nat_configure(
    internal_net: *const u8,  // 4 bytes
    internal_mask: *const u8, // 4 bytes
    external_ip: *const u8,   // 4 bytes
) -> i32 {
    let table = match crate::get_nat_table() {
        Some(t) => t,
        None => return -1,
    };

    unsafe {
        if internal_net.is_null() || internal_mask.is_null() || external_ip.is_null() {
            return -1;
        }

        let mut config = table::NatConfig::default();
        core::ptr::copy_nonoverlapping(internal_net, config.internal_network.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(internal_mask, config.internal_netmask.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(external_ip, config.external_ip.as_mut_ptr(), 4);

        table.set_config(config);
    }

    0
}
