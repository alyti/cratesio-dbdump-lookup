+++
title = "bevy-mockup-crate"
description = "Yeet"

[taxonomies]
categories = ["2D"] # these gotta be defined by human

[extra]
link = "https://github.com/frewsxcv/bevy-earcutr" # i _could_ get that from crates table...

# populated by the tool

latest_bevy_supported = "^0.5"
latest_version = "0.3.0"
latest_license = "MIT"

[[release]]
bevy_version = "bevy = ^0.5"
version = "0.3.0"
license = "MIT"

[[release]]
bevy_version = "bevy = ^0.4"
version = "0.2.0"
license = "MIT"

[[release]]
bevy_version = "bevy_ecs = ^0.4" # some crates don't depend on main bevy crate and opt for sub-crate... on the other hand need to find all the subscrates then...
version = "0.1.0"
license = "MIT"

+++