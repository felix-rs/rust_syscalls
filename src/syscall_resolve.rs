use core::arch::asm;
use core::ptr::addr_of;

use core::slice;

use ntapi::ntldr::PLDR_DATA_TABLE_ENTRY;
use ntapi::ntpebteb::TEB;
use ntapi::ntpsapi::PPEB_LDR_DATA;
use ntapi::FIELD_OFFSET;

use winapi::shared::minwindef::{PUSHORT, PWORD};
use winapi::shared::ntdef::{NULL, PLIST_ENTRY, PUCHAR, PVOID, ULONG};
use winapi::um::winnt::{
    PIMAGE_DATA_DIRECTORY, PIMAGE_DOS_HEADER, PIMAGE_EXPORT_DIRECTORY, PIMAGE_NT_HEADERS,
};

use crate::obf::dbj2_hash;

#[cfg(target_arch = "x86_64")]
#[doc(hidden)]
unsafe fn __readgsqword(offset: u32) -> u64 {
    let out: u64;
    asm!(
    "mov {}, gs:[{:e}]",
    lateout(reg) out,
    in(reg) offset,
    options(nostack, pure, readonly),
    );
    out
}

#[cfg(target_arch = "x86")]
#[doc(hidden)]
unsafe fn __readfsdword(offset: u32) -> u32 {
    let out: u32;
    asm!(
    "mov {:e}, fs:[{:e}]",
    lateout(reg) out,
    in(reg) offset,
    options(nostack, pure, readonly),
    );
    out
}

#[cfg(target_arch = "x86")]
fn is_wow64() -> bool {
    let addr = unsafe { __readfsdword(0xC0) };
    addr != 0
}

fn nt_current_teb() -> *mut TEB {
    use winapi::um::winnt::NT_TIB;
    let teb_offset = FIELD_OFFSET!(NT_TIB, _Self) as u32;

    #[cfg(target_arch = "x86_64")]
    unsafe {
        __readgsqword(teb_offset) as *mut TEB
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        __readfsdword(teb_offset) as *mut TEB
    }
}

fn get_cstr_len(pointer: *const char) -> usize {
    let mut tmp: u64 = pointer as u64;
    unsafe {
        while *(tmp as *const u8) != 0 {
            tmp += 1;
        }
    }
    (tmp - pointer as u64) as _
}

fn get_module_addr(hash: ULONG) -> PVOID {
    let ldr: PPEB_LDR_DATA;
    let header: PLIST_ENTRY;
    let mut dt_entry: PLDR_DATA_TABLE_ENTRY;
    let mut entry: PLIST_ENTRY;
    let mut mod_hash: ULONG;
    let mut mod_name: &[u8];
    let mut mod_len: usize;

    unsafe {
        ldr = (*(*nt_current_teb()).ProcessEnvironmentBlock).Ldr;
        header = addr_of!((*ldr).InLoadOrderModuleList) as PLIST_ENTRY;
        entry = (*header).Flink;

        while header as u64 != entry as u64 {
            dt_entry = entry as PLDR_DATA_TABLE_ENTRY;
            mod_len = ((*dt_entry).BaseDllName.Length) as usize;
            mod_name = slice::from_raw_parts((*dt_entry).BaseDllName.Buffer as *const u8, mod_len);
            mod_hash = dbj2_hash(mod_name) as ULONG;

            if mod_hash == hash {
                return (*dt_entry).DllBase;
            }

            entry = (*entry).Flink;
        }
    }
    NULL
}

fn get_function_addr(mdoule_addr: PVOID, hash: u32) -> PVOID {
    let nt_header: PIMAGE_NT_HEADERS;
    let data_dir: PIMAGE_DATA_DIRECTORY;
    let exp_dir: PIMAGE_EXPORT_DIRECTORY;
    let addr_funcs: PWORD;
    let addr_names: PWORD;
    let addr_ords: PUSHORT;
    let mut str_addr: PUCHAR;
    let mut str_len: usize;
    let addr_list: &[u32];
    let name_list: &[u32];
    let ord_list: &[u16];

    let dos_header: PIMAGE_DOS_HEADER = mdoule_addr as PIMAGE_DOS_HEADER;

    unsafe {
        nt_header = (dos_header as u64 + (*dos_header).e_lfanew as u64) as PIMAGE_NT_HEADERS;
        data_dir = addr_of!((*nt_header).OptionalHeader.DataDirectory[0]) as PIMAGE_DATA_DIRECTORY;

        if (*data_dir).VirtualAddress != 0 {
            exp_dir =
                (dos_header as u64 + (*data_dir).VirtualAddress as u64) as PIMAGE_EXPORT_DIRECTORY;
            addr_funcs = (dos_header as u64 + (*exp_dir).AddressOfFunctions as u64) as PWORD;
            addr_names = (dos_header as u64 + (*exp_dir).AddressOfNames as u64) as PWORD;
            addr_ords = (dos_header as u64 + (*exp_dir).AddressOfNameOrdinals as u64) as PUSHORT;

            name_list =
                slice::from_raw_parts(addr_names as *const u32, (*exp_dir).NumberOfNames as usize);
            ord_list =
                slice::from_raw_parts(addr_ords as *const u16, (*exp_dir).NumberOfNames as usize);
            addr_list =
                slice::from_raw_parts(addr_funcs as *const u32, (*exp_dir).NumberOfNames as usize);

            for iter in 0..(*exp_dir).NumberOfNames as usize {
                str_addr = (dos_header as u64 + name_list[iter] as u64) as PUCHAR;
                str_len = get_cstr_len(str_addr as _);
                if hash == dbj2_hash(slice::from_raw_parts(str_addr as _, str_len)) {
                    return (dos_header as u64 + addr_list[ord_list[iter] as usize] as u64)
                        as PVOID;
                }
            }
        }
    }

    NULL
}

#[cfg(target_arch = "x86_64")]
#[cfg(all(feature = "direct", not(feature = "indirect")))]
pub fn get_ssn(hash: u32) -> u16 {
    let ssn: u16;

    let ntdll_addr: PVOID = get_module_addr(crate::obf!("ntdll.dll"));
    let funct_addr: PVOID = get_function_addr(ntdll_addr, hash);
    unsafe {
        ssn = *((funct_addr as u64 + 4) as *const u16);
    }
    ssn
}

#[cfg(target_arch = "x86_64")]
#[cfg(all(feature = "indirect", not(feature = "direct")))]
pub fn get_ssn(hash: u32) -> (u16, u64) {
    let ntdll_addr: PVOID;
    let funct_addr: PVOID;
    let ssn_addr: u64;
    let ssn: u16;

    ntdll_addr = get_module_addr(crate::obf!("ntdll.dll"));
    funct_addr = get_function_addr(ntdll_addr, hash);

    unsafe {
        ssn = *((funct_addr as u64 + 4) as *const u16);
    }
    ssn_addr = funct_addr as u64 + 0x12;

    (ssn, ssn_addr)
}

#[cfg(target_arch = "x86")]
#[cfg(all(feature = "direct", not(feature = "indirect")))]
pub fn get_ssn(hash: u32) -> u16 {
    let ntdll_addr: PVOID;
    let funct_addr: PVOID;
    let ssn: u16;

    ntdll_addr = get_module_addr(crate::obf!("ntdll.dll"));
    funct_addr = get_function_addr(ntdll_addr, hash);
    unsafe {
        ssn = *((funct_addr as u64 + 1) as *const u16);
    }
    ssn
}

#[cfg(target_arch = "x86")]
#[cfg(all(feature = "indirect", not(feature = "direct")))]
pub fn get_ssn(hash: u32) -> (u16, u32) {
    let ntdll_addr: PVOID;
    let funct_addr: PVOID;
    let ssn_addr: u32;
    let ssn: u16;

    ntdll_addr = get_module_addr(crate::obf!("ntdll.dll"));
    funct_addr = get_function_addr(ntdll_addr, hash);
    unsafe {
        ssn = *((funct_addr as u64 + 1) as *const u16);

        if is_wow64() {
            ssn_addr = funct_addr as u32 + 0x0A;
        } else {
            ssn_addr = funct_addr as u32 + 0x0F;
        }
    }
    (ssn, ssn_addr)
}
