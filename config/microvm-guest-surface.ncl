# Guest kernel driver-surface deny list for issue #40.
#
# Classes marked `enforce = true` fail the guest-image check when the kernel
# compiles the class in (`=y`) or the guest module closure ships a matching
# module. Classes marked `enforce = false` are measured only: the current
# NixOS-default guest kernel builds USB core and DRM/FB support in, so they are
# recorded misses that require a reviewed kernel redesign before enforcement.
let NonEmpty =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Strings =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value > 0 && std.array.all NonEmpty value
  )
in
let Class = {
  name | NonEmpty,
  enforce | Bool,
  symbols | Strings,
  module_fragments | Strings,
  rationale | NonEmpty,
}
in
{
  schema = "kiln.microvm.guest-surface.v1",
  classes = [
    {
      name = "bluetooth",
      enforce = true,
      symbols = ["CONFIG_BT"],
      module_fragments = ["bluetooth", "btusb", "btintel", "btmrvl", "btrtl"],
      rationale = "No Bluetooth transport exists in the guest; drivers are unused attack surface.",
    },
    {
      name = "wireless",
      enforce = true,
      symbols = ["CONFIG_CFG80211", "CONFIG_MAC80211"],
      module_fragments = ["cfg80211", "mac80211", "iwlwifi", "ath", "rtl"],
      rationale = "The guest has no network device other than loopback.",
    },
    {
      name = "firewire",
      enforce = true,
      symbols = ["CONFIG_FIREWIRE"],
      module_fragments = ["firewire"],
      rationale = "No IEEE 1394 controller is exposed to the guest.",
    },
    {
      name = "thunderbolt",
      enforce = true,
      symbols = ["CONFIG_THUNDERBOLT", "CONFIG_USB4"],
      module_fragments = ["thunderbolt"],
      rationale = "No Thunderbolt/USB4 controller is exposed to the guest.",
    },
    {
      name = "infiniband",
      enforce = true,
      symbols = ["CONFIG_INFINIBAND"],
      module_fragments = ["infiniband", "rdma", "ib_"],
      rationale = "No RDMA device is exposed to the guest.",
    },
    {
      name = "sound",
      enforce = true,
      symbols = ["CONFIG_SOUND", "CONFIG_SND"],
      module_fragments = ["snd", "sound"],
      rationale = "No audio device is exposed to the guest.",
    },
    {
      name = "usb_host_controllers",
      enforce = true,
      symbols = [
        "CONFIG_USB_XHCI_HCD",
        "CONFIG_USB_EHCI_HCD",
        "CONFIG_USB_OHCI_HCD",
        "CONFIG_USB_UHCI_HCD",
        "CONFIG_USB_STORAGE",
        "CONFIG_USB_HID",
      ],
      module_fragments = ["xhci", "ehci", "ohci", "uhci", "usbhid", "usb-storage"],
      rationale = "No USB controller is exposed; only the virtio source drive exists.",
    },
    {
      name = "usb_core",
      enforce = false,
      symbols = ["CONFIG_USB"],
      module_fragments = ["usbcore"],
      rationale = "The pinned NixOS-default guest kernel compiles USB core in; measured, not yet denied. Enforcing this requires the reviewed slimmed-kernel redesign.",
    },
    {
      name = "display",
      enforce = false,
      symbols = ["CONFIG_DRM", "CONFIG_FB", "CONFIG_FRAMEBUFFER_CONSOLE", "CONFIG_BACKLIGHT_CLASS_DEVICE", "CONFIG_LCD_CLASS_DEVICE"],
      module_fragments = ["drm", "fb", "backlight"],
      rationale = "The pinned NixOS-default guest kernel compiles DRM and framebuffer support in; measured, not yet denied. The guest has no display device. Enforcing this requires the reviewed slimmed-kernel redesign.",
    },
  ],
}
