// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

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
        log::error!("[NAT] outbound: pkt is null");
        return -1;
    }

    let table = match crate::get_nat_table() {
        Some(t) => t,
        None => {
            log::error!("[NAT] outbound: NAT table not initialized");
            return -1;
        }
    };

    // Parse packet
    let mut ctx = match PacketContext::from_pkt(pkt) {
        Some(c) => c,
        None => {
            log::error!("[NAT] outbound: failed to parse packet");
            return -1;
        }
    };

    // Perform NAT translation
    match table.translate_outbound(&mut ctx) {
        Ok(_) => {
            // *** CRITICAL: Only apply if needs_update is true ***
            if ctx.needs_update {
                log::warn!("[NAT] outbound: Applying changes to packet");
                ctx.apply_to_pkt(pkt);
                // unsafe {
                //     net_try_send_data(pkt, K_TICKS_FOREVER);
                // }
                return 1;
            } else {
                log::info!("[NAT] outbound: No changes needed, packet unchanged");
            }
            0
        }
        Err(_) => {
            log::error!("[NAT] outbound: translation failed");
            -1
        }
    }
}

/// Process inbound packet (WAN -> LAN)
#[no_mangle]
pub extern "C" fn rust_nat_inbound(pkt: *mut NetPkt) -> i32 {
    if pkt.is_null() {
        log::error!("[NAT] inbound: pkt is null");
        return -1;
    }

    let table = match crate::get_nat_table() {
        Some(t) => t,
        None => {
            log::error!("[NAT] inbound: NAT table not initialized");
            return -1;
        }
    };

    // Parse packet
    let mut ctx = match PacketContext::from_pkt(pkt) {
        Some(c) => c,
        None => {
            log::error!("[NAT] inbound: failed to parse packet");
            return -1;
        }
    };

    // // Perform NAT translation
    match table.translate_inbound(&mut ctx) {
        Ok(_) => {
            // *** CRITICAL: Only apply if needs_update is true ***
            if ctx.needs_update {
                log::warn!("[NAT] inbound: Applying changes to packet");
                ctx.apply_to_pkt(pkt);
                return 1;
            } else {
                log::info!("[NAT] inbound: No changes needed, packet unchanged");
            }
            0
        }
        Err(_) => {
            // This is OK for inbound - no matching NAT entry
            log::info!("[NAT] inbound: no matching NAT entry (OK for new connections)");
            0 // ← Return 0, not -1!
        }
    }
}

/// Configure NAT (called from C)
#[no_mangle]
pub extern "C" fn rust_nat_configure_full(
    internal_net: *const u8,  // 4 bytes
    internal_mask: *const u8, // 4 bytes
    external_ip: *const u8,   // 4 bytes
    internal_iface: *mut NetIf,
    external_iface: *mut NetIf,
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

        config.internal_iface = internal_iface;
        config.external_iface = external_iface;

        log::info!(
            "[NAT CFG] Internal: {:p}, External: {:p}",
            internal_iface,
            external_iface
        );

        table.set_config(config);
    }

    0
}
