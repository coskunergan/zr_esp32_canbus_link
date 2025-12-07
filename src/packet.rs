//! Packet manipulation helpers

use crate::ffi::*;
use core::mem;
use core::ptr;

pub struct PacketContext {
    pub ip_hdr: Ipv4Hdr,
    pub src_port: u16,
    pub dst_port: u16,
    pub needs_update: bool,
}

impl PacketContext {
    /// Parse packet from Zephyr net_pkt (SAFE VERSION)
    pub fn from_pkt(pkt: *mut NetPkt) -> Option<Self> {
        if pkt.is_null() {
            log::error!("[NAT] ERROR: pkt is null\n");
            return None;
        }

        unsafe {
            // Get raw packet buffer pointer
            let buf_ptr = net_pkt_get_buffer(pkt);
            if buf_ptr.is_null() {
                return None;
            }

            // Read IPv4 header (aligned read)
            let ip_hdr_slice = core::slice::from_raw_parts(buf_ptr, mem::size_of::<Ipv4Hdr>());
            let mut ip_hdr = Ipv4Hdr {
                vhl: 0,
                tos: 0,
                len: [0; 2],
                id: [0; 2],
                offset: [0; 2],
                ttl: 0,
                proto: 0,
                chksum: 0,
                src: [0; 4],
                dst: [0; 4],
            };

            // Safely copy header bytes
            if ip_hdr_slice.len() < mem::size_of::<Ipv4Hdr>() {
                return None;
            }

            // Manually parse to avoid alignment issues
            ip_hdr.vhl = ip_hdr_slice[0];
            ip_hdr.tos = ip_hdr_slice[1];
            ip_hdr.len[0] = ip_hdr_slice[2];
            ip_hdr.len[1] = ip_hdr_slice[3];
            ip_hdr.id[0] = ip_hdr_slice[4];
            ip_hdr.id[1] = ip_hdr_slice[5];
            ip_hdr.offset[0] = ip_hdr_slice[6];
            ip_hdr.offset[1] = ip_hdr_slice[7];
            ip_hdr.ttl = ip_hdr_slice[8];
            ip_hdr.proto = ip_hdr_slice[9];
            ip_hdr.chksum = u16::from_be_bytes([ip_hdr_slice[10], ip_hdr_slice[11]]);
            ip_hdr.src.copy_from_slice(&ip_hdr_slice[12..16]);
            ip_hdr.dst.copy_from_slice(&ip_hdr_slice[16..20]);

            // Validate header
            if (ip_hdr.vhl >> 4) != 4 {
                // Not IPv4
                return None;
            }

            // Calculate header length
            let ihl = (ip_hdr.vhl & 0x0F) as usize * 4;
            if ihl < 20 || ihl > 60 {
                // Invalid IHL
                return None;
            }

            // Get transport layer header
            let l4_offset = ihl;
            let l4_slice = core::slice::from_raw_parts(buf_ptr.add(l4_offset), 8);

            let (src_port, dst_port) = match ip_hdr.proto {
                6 | 17 => {
                    // TCP or UDP - both have ports at same offset
                    if l4_slice.len() < 4 {
                        return None;
                    }
                    let src = u16::from_be_bytes([l4_slice[0], l4_slice[1]]);
                    let dst = u16::from_be_bytes([l4_slice[2], l4_slice[3]]);
                    (src, dst)
                }
                _ => {
                    // Other protocols (ICMP, etc.)
                    (0, 0)
                }
            };

            Some(Self {
                ip_hdr,
                src_port,
                dst_port,
                needs_update: false,
            })
        }
    }

    /// Apply changes back to packet (SAFE VERSION)
    pub fn apply_to_pkt(&self, pkt: *mut NetPkt) {
        if !self.needs_update || pkt.is_null() {
            return;
        }

        unsafe {
            let buf_ptr = net_pkt_get_buffer(pkt);
            if buf_ptr.is_null() {
                return;
            }

            // Update IP header fields
            let ip_slice = core::slice::from_raw_parts_mut(buf_ptr, 20);

            ip_slice[0] = self.ip_hdr.vhl;
            ip_slice[1] = self.ip_hdr.tos;
            ip_slice[2] = self.ip_hdr.len[0];
            ip_slice[3] = self.ip_hdr.len[1];
            ip_slice[4] = self.ip_hdr.id[0];
            ip_slice[5] = self.ip_hdr.id[1];
            ip_slice[6] = self.ip_hdr.offset[0];
            ip_slice[7] = self.ip_hdr.offset[1];
            ip_slice[8] = self.ip_hdr.ttl;
            ip_slice[9] = self.ip_hdr.proto;

            // Checksum (leave as is for now, will recalculate later)
            let chksum_bytes = self.ip_hdr.chksum.to_be_bytes();
            ip_slice[10] = chksum_bytes[0];
            ip_slice[11] = chksum_bytes[1];

            ip_slice[12..16].copy_from_slice(&self.ip_hdr.src);
            ip_slice[16..20].copy_from_slice(&self.ip_hdr.dst);

            // Update transport header ports
            let ihl = (self.ip_hdr.vhl & 0x0F) as usize * 4;
            let l4_slice = core::slice::from_raw_parts_mut(buf_ptr.add(ihl), 4);

            match self.ip_hdr.proto {
                6 | 17 => {
                    // TCP or UDP
                    let src_bytes = self.src_port.to_be_bytes();
                    let dst_bytes = self.dst_port.to_be_bytes();

                    l4_slice[0] = src_bytes[0];
                    l4_slice[1] = src_bytes[1];
                    l4_slice[2] = dst_bytes[0];
                    l4_slice[3] = dst_bytes[1];
                }
                _ => {}
            }

            // Mark packet as modified
            net_pkt_set_modified(pkt);
        }
    }
}
