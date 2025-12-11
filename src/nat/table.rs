// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

use super::entry::{NatEntry, Protocol};
use crate::ffi::*;
use crate::nat::NetIf;
use crate::packet::PacketContext;
use heapless::Vec;

const MAX_NAT_ENTRIES: usize = 128;
const PORT_RANGE_START: u16 = 50000;
const PORT_RANGE_END: u16 = 65535;

static mut PEAK_NAT_USAGE: usize = 0;

/// NAT configuration
pub struct NatConfig {
    /// Internal (LAN) network - packets FROM this network will be NAT'd
    pub internal_network: [u8; 4],
    pub internal_netmask: [u8; 4],

    /// External (WAN) IP - our public-facing IP
    pub external_ip: [u8; 4],

    /// Internal (AP) interface pointer
    pub internal_iface: *mut NetIf,

    /// External (STA) interface pointer
    pub external_iface: *mut NetIf,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            // LAN: 192.168.4.0/24 (AP subnet)
            internal_network: [192, 168, 4, 0],
            internal_netmask: [255, 255, 255, 0],

            // WAN: 192.168.1.77 (STA IP on main network)
            external_ip: [192, 168, 1, 77],

            internal_iface: core::ptr::null_mut(),
            external_iface: core::ptr::null_mut(),
        }
    }
}

use core::fmt::Write;
use heapless::String;

fn format_ip(ip: &[u8; 4]) -> String<16> {
    let mut s: String<16> = String::new();
    let _ = write!(s, "{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
    s
}

pub struct NatTable {
    entries: Vec<NatEntry, MAX_NAT_ENTRIES>,
    next_port: u16,
    config: NatConfig,
}

impl NatTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_port: PORT_RANGE_START,
            config: NatConfig::default(),
        }
    }

    fn update_peak_usage(&mut self) -> usize {
        let current = self.entries.len();

        unsafe {
            if current > PEAK_NAT_USAGE {
                PEAK_NAT_USAGE = current;
                log::info!(
                    "[NAT STATS] New Connection: {} / {} (Peak {})",
                    current,
                    MAX_NAT_ENTRIES,
                    PEAK_NAT_USAGE
                );
            }
        }

        current
    }

    /// Check if IP is in internal network (should be NAT'd)
    fn is_internal_ip(&self, ip: &[u8; 4]) -> bool {
        for i in 0..4 {
            if (ip[i] & self.config.internal_netmask[i])
                != (self.config.internal_network[i] & self.config.internal_netmask[i])
            {
                return false;
            }
        }
        true
    }

    /// Check if IP is our external IP (WAN interface)
    fn is_external_ip(&self, ip: &[u8; 4]) -> bool {
        ip == &self.config.external_ip
    }

    /// Get current uptime (mock - should call Zephyr's k_uptime_get_32)
    fn get_uptime() -> u32 {
        // TODO: Call Zephyr k_uptime_get_32() via FFI
        unsafe {
            return ffi_k_uptime_get_32();
        }
    }

    /// Allocate a new external port
    fn allocate_port(&mut self) -> u16 {
        let port = self.next_port;
        self.next_port += 1;
        if self.next_port > PORT_RANGE_END {
            self.next_port = PORT_RANGE_START;
        }
        port
    }

    /// Find existing NAT entry for outbound packet
    fn find_outbound(
        &self,
        src_ip: &[u8; 4],
        src_port: u16,
        dst_ip: &[u8; 4],
        dst_port: u16,
        proto: Protocol,
    ) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.matches_outbound(src_ip, src_port, dst_ip, dst_port, proto))
    }

    /// Find existing NAT entry for inbound packet
    fn find_inbound(
        &self,
        src_ip: &[u8; 4],
        src_port: u16,
        dst_port: u16,
        proto: Protocol,
    ) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.matches_inbound(src_ip, src_port, dst_port, proto))
    }

    /// Clean up expired entries
    fn cleanup(&mut self) {        
        let now = Self::get_uptime(); // orj
        let before = self.entries.len();

        // Expired olmayanlar kalsın
        self.entries.retain(|e| !e.is_expired(now)); // orj

        let after = self.entries.len();
        let removed = before - after;

        if removed > 0 {
            log::error!(
                "[NAT] cleanup: {} expired entry removed. ({} -> {})",
                removed,
                before,
                after
            );
        }
    }

    /// Translate outbound packet (LAN -> WAN)
    /// Only translate if source is from internal network
    pub fn translate_outbound(&mut self, ctx: &mut PacketContext) -> Result<(), ()> {
        let src_internal = self.is_internal_ip(&ctx.ip_hdr.src);
        let dst_internal = self.is_internal_ip(&ctx.ip_hdr.dst);

        // log::info!(
        //     "[NAT OUT] proto={} src={}.{}.{}.{}:{} dst={}.{}.{}.{}:{} iface={:p}",
        //     ctx.ip_hdr.proto,
        //     ctx.ip_hdr.src[0],
        //     ctx.ip_hdr.src[1],
        //     ctx.ip_hdr.src[2],
        //     ctx.ip_hdr.src[3],
        //     ctx.src_port,
        //     ctx.ip_hdr.dst[0],
        //     ctx.ip_hdr.dst[1],
        //     ctx.ip_hdr.dst[2],
        //     ctx.ip_hdr.dst[3],
        //     ctx.dst_port,
        //     ctx.iface
        // );

        // Policy checks
        if !src_internal {
            // log::info!("[NAT OUT] ✓ PASS-THROUGH: Source not internal");
            return Ok(());
        }

        if dst_internal {
            // log::info!("[NAT OUT] ✓ PASS-THROUGH: Destination is internal");
            return Ok(());
        }

        let dst = ctx.ip_hdr.dst;

        // Multicast (224.0.0.0 – 239.255.255.255)
        if (224..=239).contains(&dst[0]) {
            //log::warn!("[NAT OUT] SKIP: Multicast packet");
            return Ok(());
        }

        // Global broadcast
        if dst == [255, 255, 255, 255] {
            //log::warn!("[NAT OUT] SKIP: Global broadcast");
            return Ok(());
        }

        // Local broadcast (x.x.x.255)
        // LAN broadcast (örn: 192.168.4.255)
        let lan_ip = [192, 168, 4, 1];
        let netmask = [255, 255, 255, 0];

        let mut broadcast = [0u8; 4];
        for i in 0..4 {
            broadcast[i] = (lan_ip[i] & netmask[i]) | (!netmask[i]);
        }

        if dst == broadcast {
            //log::warn!("[NAT OUT] SKIP LAN broadcast");
            return Ok(());
        }

        // log::info!("[NAT OUT] ✓ TRANSLATING...");

        let proto = Protocol::from_u8(ctx.ip_hdr.proto).ok_or(())?;

        // log::info!("[NAT OUT] ✓ TRANSLATING STEP-2...");

        // Check if we already have an entry
        if let Some(idx) = self.find_outbound(
            &ctx.ip_hdr.src,
            ctx.src_port,
            &ctx.ip_hdr.dst,
            ctx.dst_port,
            proto,
        ) {
            //log::error!("[NAT] outbound: VAR olan bir NAT paketi bulundu <<<<<<<");
            // Update existing entry
            let entry = &mut self.entries[idx];
            entry.touch(Self::get_uptime());

            // Translate
            ctx.ip_hdr.src = entry.external_ip;
            ctx.src_port = entry.external_port;

            // *** CHANGE INTERFACE ***
            if !entry.external_iface.is_null() {
                // log::info!(
                //     "[NAT OUT] ✓ Interface change: {:p} -> {:p} (existing entry)",
                //     ctx.iface,
                //     entry.external_iface
                // );
                ctx.iface = entry.external_iface;
            }

            ctx.needs_update = true;

            // log::info!(
            //     "[NAT OUT] ✓ Existing entry: {}.{}.{}.{}:{} -> {}.{}.{}.{}:{}",
            //     entry.internal_ip[0],
            //     entry.internal_ip[1],
            //     entry.internal_ip[2],
            //     entry.internal_ip[3],
            //     entry.internal_port,
            //     entry.external_ip[0],
            //     entry.external_ip[1],
            //     entry.external_ip[2],
            //     entry.external_ip[3],
            //     entry.external_port
            // );

            return Ok(());
        }

        // Create new entry
        self.cleanup(); // Make room if needed

        let external_port = if proto == Protocol::Icmp {
            0
        } else {
            self.allocate_port()
        };

        let external_ip = self.config.external_ip;

        let mut entry = NatEntry::new();
        entry.internal_ip = ctx.ip_hdr.src; // 4.100
        entry.internal_port = ctx.src_port; //1
        entry.external_ip = external_ip; // 1.77
        entry.external_port = external_port; // 50003
        entry.remote_ip = ctx.ip_hdr.dst;
        entry.remote_port = ctx.dst_port; // 50003
        entry.protocol = proto; // 1
        entry.last_activity = Self::get_uptime();
        entry.in_use = true;

        // *** STORE INTERFACE POINTERS ***
        entry.internal_iface = ctx.orig_iface;
        entry.external_iface = self.config.external_iface;

        //log::info!("[NAT OUT] ✓ TRANSLATING STEP-3...");
        // Add to table
        self.entries.push(entry).map_err(|_| ())?;

        //log::info!("[NAT OUT] ✓ TRANSLATING STEP-4...");

        // Translate packet
        ctx.ip_hdr.src = external_ip;
        ctx.src_port = external_port;

        // *** CHANGE INTERFACE ***
        if !self.config.external_iface.is_null() {
            // log::info!(
            //     "[NAT OUT] ⚡ Interface change: {:p} -> {:p} (new entry)",
            //     ctx.iface,
            //     self.config.external_iface
            // );
            ctx.iface = self.config.external_iface;
        } else {
            log::error!("[NAT OUT] External interface not configured!");
        }

        ctx.needs_update = true;

        let current_usage = self.update_peak_usage();
        log::info!(
            "[NAT OUT] Online Connection: {} / {} (Peak: {})",
            current_usage,
            MAX_NAT_ENTRIES,
            unsafe { PEAK_NAT_USAGE }
        );

        // log::info!(
        //     "[NAT OUT] ✓ New entry: {}.{}.{}.{}:{} -> {}.{}.{}.{}:{} [port {}]",
        //     entry.internal_ip[0],
        //     entry.internal_ip[1],
        //     entry.internal_ip[2],
        //     entry.internal_ip[3],
        //     entry.internal_port,
        //     entry.external_ip[0],
        //     entry.external_ip[1],
        //     entry.external_ip[2],
        //     entry.external_ip[3],
        //     entry.external_port,
        //     external_port
        // );

        Ok(())
    }

    /// Translate inbound packet (WAN -> LAN)
    /// Only translate if destination is our external IP
    pub fn translate_inbound(&mut self, ctx: &mut PacketContext) -> Result<(), ()> {
        if !self.is_external_ip(&ctx.ip_hdr.dst) {
            // Not for us, don't NAT
            return Ok(());
        }

        //   if ctx.ip_hdr.src != [192, 168, 4, 100] {
        //         //log::warn!("[NAT OUT] SKIP: Source is AP gateway itself");
        //         return Ok(());
        //     }

        let proto = Protocol::from_u8(ctx.ip_hdr.proto).ok_or(())?;

        // Find matching entry
        let idx = self
            .find_inbound(&ctx.ip_hdr.src, ctx.src_port, ctx.dst_port, proto)
            .ok_or(())?;

        let entry = &mut self.entries[idx];
        entry.touch(Self::get_uptime());

        // Translate destination IP and port
        ctx.ip_hdr.dst = entry.internal_ip;
        ctx.dst_port = entry.internal_port;

        // *** CHANGE INTERFACE ***
        if !entry.internal_iface.is_null() {
            // log::info!(
            //     "[NAT IN] ✓ Interface change: {:p} -> {:p}",
            //     ctx.iface,
            //     entry.internal_iface
            // );
            ctx.iface = entry.internal_iface;
        }

        ctx.needs_update = true;

        // log::info!(
        //     "[NAT IN] ✓ Translated: {}.{}.{}.{}:{} -> {}.{}.{}.{}:{}",
        //     entry.external_ip[0],
        //     entry.external_ip[1],
        //     entry.external_ip[2],
        //     entry.external_ip[3],
        //     entry.external_port,
        //     entry.internal_ip[0],
        //     entry.internal_ip[1],
        //     entry.internal_ip[2],
        //     entry.internal_ip[3],
        //     entry.internal_port
        // );
        Ok(())
    }

    /// Set NAT configuration (called from C)
    pub fn set_config(&mut self, config: NatConfig) {
        self.config = config;
        let current = self.entries.len();
        unsafe {
            if current > PEAK_NAT_USAGE {
                PEAK_NAT_USAGE = current;
            }
        }
    }
}
