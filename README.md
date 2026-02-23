# Minimon COSMIC Applet

A configurable applet for displaying the following:
* CPU load
* CPU temperature
* Memory usage
* Network utilization
* Disk activity
* GPU and VRAM usage on Nvidia and AMD GPUs. 

Can sit in the panel or Dock. Configurable refresh rate and many display options.

![Image](cosmic-applet-minimon.png)


![Image](https://github.com/user-attachments/assets/5d697c74-f7dc-4213-8516-465c32e5567b)


![Image](https://github.com/user-attachments/assets/b6fa25a0-2945-4a40-bdf4-38ef946b8d26)



![Image](https://github.com/user-attachments/assets/2787cf05-2121-4c25-b1a2-d0b511c30215)

![Image](https://github.com/user-attachments/assets/fa6f4b2c-ab95-4815-b7ab-fdd7557797f7)

## Installing

### Flatpak

Depending on how you've installed COSMIC Desktop, Minimon may show up in your app store by default. In COSMIC Store it should be under the "COSMIC Applets" category.

If Minimon does not show up in your app store, you'll need to add `cosmic-flatpak` as a source:
```sh
flatpak remote-add --if-not-exists --user cosmic https://apt.pop-os.org/cosmic/cosmic.flatpakrepo
```

Then, proceed to your preferred app store and search for Minimon.

### From package manager

If on a .deb based distibution download [latest version](https://github.com/cosmic-utils/minimon-applet/releases) and install with the following commands:

```sh
sudo dpkg -i cosmic-applet-minimon_0.3.10_amd64.deb
```

### Post-installation

Once it is installed, it should show up in cosmic settings when modifying the applets on the dock or panel.

It can also be launched as a conventional application from the terminal using:
```sh
flatpak run io.github.cosmic_utils.minimon-applet
```
or launched on desktop environments outside of COSMIC. It only functions as an applet on COSMIC Desktop, however.

## Building

To build the applet, you will need [just](https://github.com/casey/just) and probably xkbcommon, if you're on Pop!\_OS, you can install it with the following command:

```sh
sudo apt install just libxkbcommon-dev
```

Run the following commands to build and install the applet:

```sh
just build-release
just install
```

Alternatively generate a deb or rpm file for installation:

```sh
just build-release
just deb
just rpm
```
and install with:

```sh
sudo dpkg -i <name_of.deb>
sudo dnf install <name_of.rpm>
```

For checking logs:

```sh
journalctl SYSLOG_IDENTIFIER=cosmic-applet-minimon
```
