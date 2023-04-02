# rshole

A (very WIP) tool and library for parsing structs from dwarf information written in rust!

I plan to add python bindings once it is more functional.

This was spawned out of the lack of an efficient parser for binaries with large amounts of dwarf information, e.g. linux kernel images, accessible to python that doesn't just eat memory until it gets OOM-killed.

The goal is to provide all of the information that pahole provides through a hopefully sane and safe api.

NOTE: This is my first rust project and I had no previous experience with dwarf when starting this so some of the code might be really bad, but it does mostly function.

## Usage Instructions

No api docs or whatever yet, but the example tool can be run as follows:

```console
$ cargo run --example rshole --release ~/linux/vmlinux
```

## Example Output

```
...
struct deferred_entry {
  list_head list
  grant_ref_t ref
  uint16_t warn_delay
  page *page
}

struct cpu_timer {
  timerqueue_node node
  timerqueue_head *head
  pid *pid
  list_head elist
  int firing
}

struct virtio_pci_device {
  virtio_device vdev
  pci_dev *pci_dev
  virtio_pci_legacy_device ldev
  virtio_pci_modern_device mdev
  bool is_legacy
  u8 *isr
  spinlock_t lock
  list_head virtqueues
  virtio_pci_vq_info **vqs
  int msix_enabled
  int intx_enabled
  cpumask_var_t *msix_affinity_masks
  *msix_names
  unsigned int msix_vectors
  unsigned int msix_used_vectors
  bool per_vq_vectors
  *setup_vq
  *del_vq
  *config_vector
}
...
```
