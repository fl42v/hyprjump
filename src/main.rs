use hyprland;
use hyprland::data as HData;
use hyprland::dispatch;
use hyprland::keyword::{Keyword, OptionValue};
use hyprland::prelude::*;

use std::cmp::max;

#[derive(Debug, Clone)]
enum Action {
    Move(bool, i32),
    // TODO: rename them!
    Focus(String),
    // TODO: (Un)Group; not really a priority, tho.
    // Moving a window in the direction of another window
    // should create a new group, and moving from an edge of said
    // group towards an edge of the workspace (monitor) sould
    // remove it from the group. #1
    // checkout swapgroup dispatcher
}

// ----------- Data gathering -----------------
// TODO: move this shit to another file

#[derive(Debug, Clone)]
struct Client {
    address: String,
    width: i16,
    height: i16,
    top_left: (/*x:*/ i16, /*y:*/ i16),
    monitor: i16,
    is_floating: bool,
    is_fullscreen: bool,
    workspace_id: i16,
    // TODO: #1
    // requires rewriting to use `Address`es
    // grouped: Vec<Box<Address>>
}

impl From<&HData::Client> for Client {
    fn from(client: &HData::Client) -> Client {
        Client {
            address: client.address.to_string(),
            width: client.size.0,
            height: client.size.1,
            top_left: client.at,
            monitor: client.monitor as i16,
            is_floating: client.floating,
            is_fullscreen: match client.fullscreen {
                HData::FullscreenMode::None => false,
                _ => true,
            },
            workspace_id: client.workspace.id as i16, // like seriously, [-32_768..32_767]
                                                      // is more than enough
        }
    }
}

#[derive(Debug, Clone)]
struct Workspace {
    id: i16,
    client_addresses: Vec<String>,
    monitor_id: i16,
}

impl Workspace {
    // I suspenct you can't pass additional parameters to `from()`
    fn wrap(workspace: &HData::Workspace, clients: &Vec<Client>, monitors: &Vec<Monitor>) -> Self {
        Self {
            id: workspace.id as i16,
            client_addresses: clients
                .iter()
                .filter(|c| c.workspace_id == workspace.id as i16)
                .map(|c| c.address.to_string())
                .collect(),
            monitor_id: monitors
                .iter()
                .filter(|m| m.name == workspace.monitor)
                .collect::<Vec<_>>()[0]
                .id as i16,
        }
    }
}

#[derive(Debug, Clone)]
struct Monitor {
    id: i16,
    name: String,
    width: i16,
    height: i16,
    x: i16,
    y: i16,
    active_workspace_id: i16,
    //possibly fixes the issue with bars:
    //reserved: (u16, u16, u16, u16),
}

impl From<&HData::Monitor> for Monitor {
    fn from(monitor: &HData::Monitor) -> Monitor {
        Monitor {
            id: monitor.id as i16,
            name: monitor.name.clone(),
            width: monitor.width as i16,
            height: monitor.height as i16,
            x: monitor.x as i16,
            y: monitor.y as i16,
            active_workspace_id: monitor.active_workspace.id as i16,
        }
    }
}

#[derive(Debug, Clone)]
struct State {
    active_window_address: String, // tecnhically can be 0x0,
    // in which case search returns nothing
    active_workspace_id: i16,
    active_monitor_whatever_the_fuck_it_is: String,
    clients: Vec<Client>,
    workspaces: Vec<Workspace>,
    monitors: Vec<Monitor>,
    is_vertical: bool,
}

impl State {
    fn new() -> Self {
        let clients = HData::Clients::get()
            .expect("Unable to obtain the list of clients")
            .to_vec();
        let clients: Vec<Client> = clients
            .iter()
            .map(|c| Client::from(c))
            .filter(|c| c.workspace_id > 0) // #3
            .collect();

        let monitors = HData::Monitors::get()
            .expect("Unable to obtain the list of monitors")
            .to_vec();
        let monitors: Vec<Monitor> = monitors.iter().map(|m| Monitor::from(m)).collect();

        let workspaces = HData::Workspaces::get()
            .expect("Unable to obtain the list of workspaces")
            .to_vec();
        let workspaces: Vec<Workspace> = workspaces
            .iter()
            .map(|w| Workspace::wrap(w, &clients, &monitors))
            .filter(|w| w.id > 0) // TODO: because of reasons? #3
            .collect();

        // a workaround, since Client::get_active() doesn't seem to work :/
        // TODO: works incorrectly when the actual active window is on a special
        // Workspace since it does not count as active for some weird reason
        let active_workspace = HData::Workspace::get_active().unwrap();
        dbg!(&active_workspace);

        let animations = HData::Animations::get().unwrap();
        let is_vertical =
            if let Some(wsp_anim) = animations.0.iter().find(|anim| anim.name == "workspaces") {
                match &wsp_anim.style {
                    HData::AnimationStyle::SlideVert | HData::AnimationStyle::SlideFadeVert => true,
                    HData::AnimationStyle::Unknown(s) => s.ends_with("vert"),
                    _ => false,
                }
            } else {
                false
            };

        Self {
            active_window_address: active_workspace.last_window.to_string(),
            active_workspace_id: active_workspace.id as i16,
            clients,
            workspaces,
            monitors,
            is_vertical,

            active_monitor_whatever_the_fuck_it_is: active_workspace.monitor,
        }
    }

    fn find_workspace_by_id(workspaces: &Vec<Workspace>, id: i16) -> Option<&Workspace> {
        let workspaces: Vec<&Workspace> = workspaces
            .iter()
            // does it copy, tho?
            .filter(|w| w.id == id)
            .collect();
        if workspaces.len() != 1 {
            None
        } else {
            Some(workspaces[0])
        }
    }

    fn next_monitor_in_the_direction(&self, direction: &str) -> Option<&Monitor> {
        // likely wouldn't work for esoteric setups
        //let active =

        let monitors: Vec<&Monitor> = self
            .monitors
            .iter()
            .filter(|m| match direction {
                // compare to that of active
                // monitor; consider moving the match
                // outside of the filter
                "u" => m.x > 5,
                "d" => m.x > 5,
                "l" => m.y > 5,
                "r" => m.y > 5,
                _ => panic!("wrong direction specified"),
            })
            .collect();
        None
    }

    fn find_window_by_address(&self, address: &String) -> Option<&Client> {
        self.clients.iter().find(|&c| &c.address == address)
    }

    fn find_clients_on_workspace(&self, workspace_id: i16) -> Vec<&Client> {
        self.clients
            .iter()
            .filter(|&c| c.workspace_id == workspace_id)
            .collect()
    }

    fn find_monitor_by_client(&self, client: &Client) -> Option<&Monitor> {
        self.monitors.iter().find(|&m| m.id == client.monitor)
    }

    // sometimes (when?) doesn't work
    fn client_has_neighbours_in_direction(&self, client: &Client, direction: &str) -> bool {
        // would've been more logical to return the next window, but we can't swap
        // 2 given windows anyways; make a plugin?
        let neighbours = self.find_clients_on_workspace(client.workspace_id);
        let neighbour = neighbours.iter().find(|&n| match direction {
            "u" => n.top_left.1 < client.top_left.1,
            "d" => {
                n.top_left.1 > client.top_left.1
                    && n.top_left.0 <= client.top_left.0
                    && n.top_left.0 + n.width >= client.top_left.0 + client.width
            }
            "l" => n.top_left.0 < client.top_left.0,
            "r" => n.top_left.0 > client.top_left.0,
            _ => panic!("wrong direction specified"),
        });
        match neighbour {
            Option::None => false,
            Option::Some(_) => true,
        }
    }
}

fn determine_action(state: &State, direction: String) -> Action {
    // TODO: fails if a tiled window
    // has no border. Parse window rules?
    let (to_workspace, relative) = match &direction[..] {
        "u" => (state.is_vertical, -1),
        "d" => (state.is_vertical, 1),
        "l" => (!state.is_vertical, -1),
        "r" => (!state.is_vertical, 1),
        _ => panic!("wrong direction specified"),
    };

    // TODO: Focusing a floating window from a tiled still does not work.
    if let Some(client) = state.find_window_by_address(&state.active_window_address) {
        if to_workspace && (client.is_floating || client.is_fullscreen) {
            return Action::Move(to_workspace, relative);
        }

        if state.client_has_neighbours_in_direction(&client, &direction) {
            return Action::Focus(direction);
        }
    }
    return Action::Move(to_workspace, relative);
}

fn do_stuff(action: Action, cmd1: &str, cmd2: &str, cmd3: &str, mod3: &str) {
    let dir; // to store an otherwise temporary value. TODO: Looks like crap. Avoidable?
    let cmd;
    let dispatcher = match action {
        Action::Focus(direction) => {
            dir = direction;
            cmd = cmd1;
        }
        Action::Move(to_workspace, direction) => {
            // always output the sign; otherwise we get movetoworkspace 1
            // instead of movetoworkspace +1
            if to_workspace {
                dir = format!("{:+}", direction);
                cmd = cmd2;
            } else {
                dir = format!("{}{:+}", mod3, direction);
                cmd = cmd3;
            }
        }
    };
    dbg!(&cmd);
    dbg!(&dir);
    dispatch::Dispatch::call(dispatch::DispatchType::Custom(cmd, &dir)).unwrap();
}

fn main() -> hyprland::Result<()> {
    let state = State::new();
    let usage = "usage example: hyprjump movefocus workspace movewindow mon: u";

    // TODO: clap
    let cmd1 = std::env::args().nth(1).expect(&usage);
    let cmd2 = std::env::args().nth(2).expect(&usage);
    let cmd3 = std::env::args().nth(3).expect(&usage);
    let mod3 = std::env::args().nth(4).expect(&usage);
    let drct = std::env::args().nth(5).expect(&usage);

    do_stuff(determine_action(&state, drct), &cmd1, &cmd2, &cmd3, &mod3);

    Ok(())
}
