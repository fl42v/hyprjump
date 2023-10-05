# Hyprspace
Implementation of [Pop's cosmic](https://github.com/pop-os/cosmic-epoch)-like window movement for [Hyprland](https://github.com/hyprwm/hyprland).
Mandatory WIP.

# What?
Keybindings that focus a window also change workspace:
if you're trying to, for example, focus the window to the left of the leftmost window,
you go to the previous workspace instead. Likewise, attempting to swap the leftmost window
with that to the left will move it to the previous workspace.

# Installation
- Once it works, there'll be a nix flake (currently a template);
- Otherwise, it's just `cargo build` and pointing to the target binary from the configuration.

# Configuration
add smth like this to your `hyprland.conf`:

```
bind=SUPER,h,exec,hyprspace movefocus l
bind=SUPER,j,exec,hyprspace movefocus d
bind=SUPER,k,exec,hyprspace movefocus u
bind=SUPER,l,exec,hyprspace movefocus r

bind=SUPERSHIFT,h,exec,hyprspace swapwindow l
bind=SUPERSHIFT,j,exec,hyprspace swapwindow d
bind=SUPERSHIFT,k,exec,hyprspace swapwindow u
bind=SUPERSHIFT,l,exec,hyprspace swapwindow r

```

## Vertical workspaces
Hyprspace determines the orientation automatically by checking which style is used for `animations:animation-workspaces`.

# What is missing
- Moving windows to other monitors;
- Restricting movement through unpopulated workspaces;
- Proper handling of the special workspace;
- A demo on this page;
- Whatever I forgot to add here but specified in the `TODO`-comments;
