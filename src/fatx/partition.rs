pub struct PartitionMapEntry {
    pub letter: &'static str,
    pub name: &'static str,
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

pub const DEFAULT_PARTITION_LAYOUT: &[PartitionMapEntry] = &[
    PartitionMapEntry {
        letter: "x",
        name: "Xbox Original X",
        offset_bytes: 0x00080000,
        size_bytes: 0x02ee00000,
    },
    PartitionMapEntry {
        letter: "y",
        name: "Xbox Original Y",
        offset_bytes: 0x2ee80000,
        size_bytes: 0x02ee00000,
    },
    PartitionMapEntry {
        letter: "z",
        name: "Xbox Original Z",
        offset_bytes: 0x5dc80000,
        size_bytes: 0x02ee00000,
    },
    PartitionMapEntry {
        letter: "c",
        name: "Xbox Original C",
        offset_bytes: 0x8ca80000,
        size_bytes: 0x01f400000,
    },
    PartitionMapEntry {
        letter: "e",
        name: "Xbox Original E (Data)",
        offset_bytes: 0xabe80000,
        size_bytes: 0x1312d6000,
    },
    // Xbox 360 Presets
    PartitionMapEntry {
        letter: "360_p1",
        name: "Xbox 360 Partition 1 (Cache)",
        offset_bytes: 0x00110000,
        size_bytes: 0x80000000, // 2GB
    },
    PartitionMapEntry {
        letter: "360_p2",
        name: "Xbox 360 Partition 2 (System)",
        offset_bytes: 0x120EB0000,
        size_bytes: 0x10000000, // 256MB
    },
    PartitionMapEntry {
        letter: "360_p3",
        name: "Xbox 360 Partition 3 (Data)",
        offset_bytes: 0x130EB0000,
        size_bytes: 0x20000000000, // 2TB max
    },
];

impl PartitionMapEntry {
    pub fn from_letter(letter: &str) -> Option<&PartitionMapEntry> {
        DEFAULT_PARTITION_LAYOUT
            .iter()
            .find(|&entry| entry.letter == letter)
    }
}
