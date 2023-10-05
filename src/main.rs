use hyprland::data as HData;
use hyprland::dispatch::Direction;
use hyprland::dispatch;
use hyprland::keyword::{Keyword, OptionValue};
use hyprland::prelude::*;
use hyprland;

use std::cmp::max;

#[derive(Debug, Clone)]
enum Action {
  Move(i32),
  // TODO: rename them!
  RenameMe(Direction),
  // TODO: (Un)Group; not really a priority, tho.
  // Moving a window in the direction of another window 
  // should create a new group, and moving from an edge of said
  // group towards an edge of the workspace (monitor) sould 
  // remove it from the group. #1
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
      is_fullscreen: client.fullscreen,
      workspace_id: client.workspace.id as i16 // like seriously, [-32_768..32_767]
                                               // is more than enough
    }
  }
}

#[derive(Debug, Clone)]
enum SmartGapsState {
  Disabled,
  NoBorder,
  WithBorder
}

impl SmartGapsState {
  // TODO: `Result`ify
  fn get() -> Self {
    match Keyword::get("dwindle:no_gaps_when_only").unwrap().value {
      // DOCS: <C-f> no_gaps_when_only https://wiki.hyprland.org/hyprland-wiki/pages/Configuring/Dwindle-Layout/
      OptionValue::Int(smart_gaps_state) => match smart_gaps_state {
                                              0 => Self::Disabled,
                                              1 => Self::NoBorder,
                                              2 => Self::WithBorder,
                                              _ => panic!("Currently impossible. New smart gaps state got introduced?")
                                            },
      _ => panic!("Unable to determine smart gaps state")
    }
  }
}

#[derive(Debug, Clone)]
// TODO: rename NewDecorations to Decorations
// when finished refactoring
struct NewDecorations {
  inner_gaps:  i16,
  outer_gaps:  i16,
  border_size: i16,
  smart_gaps_state:  SmartGapsState
}

impl NewDecorations {
  // TODO: `Result`ify
  fn get() -> Self {
    match (Keyword::get("general:gaps_in").unwrap().value,
           Keyword::get("general:gaps_out").unwrap().value,
           Keyword::get("general:border_size").unwrap().value) {
      (OptionValue::Int(inner),
       OptionValue::Int(outer),
       OptionValue::Int(bordr)) => NewDecorations {
                                     // kinda necessary to check for overflows,
                                     // but the hyprland-rs lib uses `i16`s 
                                     // for dimensions, and gaps that don't 
                                     // fit it are kinda useless anyways
                                     inner_gaps:  inner as i16,
                                     outer_gaps:  outer as i16,
                                     border_size: bordr as i16,
                                     smart_gaps_state: SmartGapsState::get(),
                                  },
      _ => panic!("Some of (gaps_in, gaps_out, border_size) are not integers. This should never happen.")
    }

  }
}

#[derive(Debug, Clone)]
struct Workspace {
  id: i16,
  client_addresses: Vec<String>,
  monitor_id: i16
}

impl Workspace {
  // I suspenct you can't pass additional parameters to `from()`
  fn wrap(workspace: &HData::Workspace, clients: &Vec<Client>, monitors: &Vec<Monitor>) -> Self {
    Self {
      id: workspace.id as i16,
      client_addresses: clients.iter()
                    .filter(|c| c.workspace_id == workspace.id as i16)
                    .map(|c| c.address.to_string())
                    .collect(),
      monitor_id: monitors.iter()
                    .filter(|m| m.name ==  workspace.monitor)
                    .collect::<Vec<_>>()[0]
                    .id as i16
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
  clients: Vec<Client>,
  workspaces: Vec<Workspace>,
  monitors: Vec<Monitor>,
  decorations: NewDecorations,
  is_vertical: bool
}

impl State {
  fn new() -> Self {
    let clients = HData::Clients::get()
      .expect("Unable to obtain the list of clients")
      .to_vec();
    let clients: Vec<Client> = clients.iter()
      .map(|c| Client::from(c))
      .filter(|c| c.workspace_id > 0) // #3
      .collect();

    let monitors = HData::Monitors::get()
      .expect("Unable to obtain the list of monitors")
      .to_vec();
    let monitors: Vec<Monitor> = monitors.iter()
      .map(|m| Monitor::from(m))
      .collect();

    let workspaces = HData::Workspaces::get()
      .expect("Unable to obtain the list of workspaces")
      .to_vec();
    let workspaces: Vec<Workspace> = workspaces.iter()
      .map(|w| Workspace::wrap(w, &clients, &monitors))
      .filter(|w| w.id > 0) // TODO: because of reasons? #3
      .collect();

    // a workaround, since Client::get_active() doesn't seem to work :/
    // TODO: works incorrectly when the actual active window is on a special
    // Workspace since it does not count as active for some weird reason
    let active_workspace = HData::Workspace::get_active().unwrap();

    let animations = HData::Animations::get().unwrap();
    let is_vertical = if let Some(wsp_anim) = animations.0.iter()
                                                .find(|anim| anim.name == "workspaces") {
      match &wsp_anim.style {
        HData::AnimationStyle::SlideVert | HData::AnimationStyle::SlideFadeVert => true,
        HData::AnimationStyle::Unknown(s) => s.ends_with("vert"),
        _ => false
      }
    } else {
      false
    };

    Self {
      active_window_address: active_workspace.last_window.to_string(),
      active_workspace_id: active_workspace.id as i16,
      clients: clients,
      workspaces: workspaces,
      monitors: monitors,
      decorations: NewDecorations::get(),
      is_vertical: is_vertical
    }
  }

  fn find_workspace_by_id(workspaces: &Vec<Workspace>, id: i16) -> Option<&Workspace> {
    let workspaces: Vec<&Workspace> = workspaces
                                        .iter()
                                        .filter(|w| w.id == id)
                                        .collect();
    if workspaces.len() != 1 {
      None
    } else {
      Some(workspaces[0])
    }
  }

  fn find_window_by_address(&self, address: &String) -> Option <&Client> {
    self.clients
      .iter()
      .find(|&c| &c.address == address)
  }

  fn find_clients_on_workspace(&self, workspace_id: i16) -> Vec<&Client> {
    self.clients
      .iter()
      .filter(|&c| c.workspace_id == workspace_id)
      .collect()
  }

  fn find_monitor_by_client(&self, client: &Client) -> Option <&Monitor> {
    self.monitors
      .iter()
      .find(|&m| m.id == client.monitor)
  }

  fn client_has_neighbours_in_direction(&self, client: &Client, direction: &Direction) -> bool {
    let neighbours = self.find_clients_on_workspace(client.workspace_id);
    let neighbour = neighbours
                    .iter()
                    .find(|&n| match direction {
                                 Direction::Up    => n.top_left.1 < client.top_left.1,
                                 Direction::Down  => n.top_left.1 > client.top_left.1,
                                 Direction::Left  => n.top_left.0 < client.top_left.0,
                                 Direction::Right => n.top_left.0 > client.top_left.0,
                               }
                    );
    match neighbour {
      Option::None => false,
      Option::Some(_) => true
    }
  }
}

// -----------------------------------------

// TODO: TF it even means? RENAME!
fn relative_direction(state: &State, direction: &Direction) -> (bool, i32) {
  let is_vert = state.is_vertical;

  match direction {
      Direction::Up    =>  (is_vert, -1),
      Direction::Down  =>  (is_vert,  1),
      Direction::Left  => (!is_vert, -1),
      Direction::Right => (!is_vert,  1),
  }
}

fn determine_action(state: &State, direction: Direction) -> Option<Action> {
  // TODO: fails if a certain non-floating window
  // has no border. Parse window rules?

  let (can_move, relative) = relative_direction(&state, &direction);

  // TODO: Focusing a floating window from a tiled still does not work.
  if let Some(client) = state.find_window_by_address(&state.active_window_address) {
  
    if can_move && (client.is_floating || client.is_fullscreen) {
        return Some(Action::Move(relative));
    }
  
    if state.client_has_neighbours_in_direction(&client, &direction) {
      Some(Action::RenameMe(direction))
    } else {
      if can_move {
        Some(Action::Move(relative))
      } else {
        None
      }
    }
  } else {
    // whatever to do when there's no active client
      Some(Action::Move(relative))
  }
}

fn do_stuff(action: Action, cmd: &str) {
  let dir; // to store an otherwise temporary value. TODO: Looks like crap. Avoidable?
  let dispatcher = match action {
    Action::RenameMe(direction) => {
      dir = direction.to_string();
      Some(dispatch::DispatchType::Custom(cmd, &dir))
    },
    Action::Move(direction) => {
        // TODO: separate logic for swap/moveto and focus/goto looks necessary
        // TODO: or next workspace is occupied
        match cmd {
          "swapwindow" => Some(dispatch::DispatchType::MoveToWorkspace(
            dispatch::WorkspaceIdentifierWithSpecial::Relative(direction), None)),
          "movefocus"  => Some(dispatch::DispatchType::Workspace(
            dispatch::WorkspaceIdentifierWithSpecial::Relative(direction))),
          //_ => panic!("Wrong dispatcher passed."),
          _ => None
        }
    },
  };
  if let Some(disp) = dispatcher {
    dispatch::Dispatch::call(disp).unwrap();
  }
}

fn main() -> hyprland::Result<()> {
  let state = State::new();

  // TODO: clap
  let cmd = std::env::args().nth(1).expect("Dispatcher not specified. Expected either swapwindow or movefocus");
  let arg    = std::env::args().nth(2).expect("No directions given. Expected one of r,d,u,l");
  let direction = match &arg[..] {
    "r" => Direction::Right,
    "d" => Direction::Down,
    "u" => Direction::Up,
    "l" => Direction::Left,
    _ => panic!("Wrong direction given. Correct are: r,d,u,l")
  };

  // tbh, can move now
  if let Some(action) = determine_action(&state, direction) {
    do_stuff(action, &cmd);
  };

  Ok(())
}
