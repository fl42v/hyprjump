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

#[derive(Debug, Clone)]
struct MonInfo {
  width: u16,
  height: u16,
  // TODO: multiple monitors setup #2
  //x: i32,
  //y: i32
}

#[derive(Debug, Clone)]
struct ClientInfo {
  width: i16,
  height: i16,
  x: i16,
  y: i16,
  monitor: MonInfo,
  floating: bool,
  fullscreen: bool,
  is_only: bool
  // TODO: #1
  //grouped: Vec<Box<Address>>
}

#[derive(Debug, Clone)]
struct Decorations {
  inner_gaps: i64,
  outer_gaps: i64,
  border_size: i64
}

// ----------- Refactoring -----------------

#[derive(Debug, Clone)]
struct Client {
  pid: i32, // since I'm kinda too lazy to compare `HData::Address`es if that's even possible
  width: i16,
  height: i16,
  top_left: (/*x:*/ i16, /*y:*/ i16),
  monitor: MonInfo,
  is_floating: bool,
  is_fullscreen: bool,
  is_only: bool
  // TODO: #1
  //grouped: Vec<Box<Address>>
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
struct Workspace {}

#[derive(Debug, Clone)]
struct State {
  active_window_pid: i32,
  active_workspace_id: i32,
  clients: Vec<Client>,
  workspaces: Vec<Workspace>,
  decorations: NewDecorations,
  is_vertical: bool
}

// -----------------------------------------

fn get_monitor_info(id: i128) -> MonInfo {
  let mon = HData::Monitors::get().unwrap().find(|m| m.id == id).unwrap();
  MonInfo {
    width: mon.width,
    height: mon.height,
    // #2
    //x: mon.x,
    //y: mon.y
  }
}


// a workaround, since Client::get_active() doesn't seem to work :/
// TODO: works incorrectly when the actual active window is on a special
// Workspace since it does not count as active for some weird reason
fn get_active_window() -> Option<ClientInfo> {
  let active_workspace = HData::Workspace::get_active().unwrap();
  let last_window = active_workspace.last_window;

  if let Some(client) = HData::Clients::get().unwrap()
    .find(|cl| cl.address.to_string() == last_window.to_string()) {
      Some(ClientInfo {
        width: client.size.0,
        height: client.size.1,
        x: client.at.0,
        y: client.at.1,
        monitor: get_monitor_info(client.monitor),
        floating: client.floating,
        fullscreen: client.fullscreen,
        is_only: (active_workspace.windows == 1)
      })
  } else {
    None
  }
}

fn vertical_workspaces() -> bool {
  let animations = HData::Animations::get().unwrap();
  if let Some(wsp_anim) = animations.0.iter().find(|anim| anim.name == "workspaces") {
    match &wsp_anim.style {
      HData::AnimationStyle::SlideVert | HData::AnimationStyle::SlideFadeVert => true,
      HData::AnimationStyle::Unknown(s) => s.ends_with("vert"),
      _ => false
    }
  } else {
    false
  }
}

fn get_decorations() -> Decorations { 
  // TODO: is it possible some of them are not specified?
  // TODO: error handling
  match (Keyword::get("general:gaps_in").unwrap().value,
      Keyword::get("general:gaps_out").unwrap().value,
      Keyword::get("general:border_size").unwrap().value) {
    (OptionValue::Int(inner),
     OptionValue::Int(outer),
     OptionValue::Int(bordr)) => Decorations {
                      inner_gaps:  inner as i64,
                      outer_gaps:  outer as i64,
                      border_size: bordr as i64
                   },
    _ => panic!("Some of (gaps_in, gaps_out, border_size) are not integers. This should never happen.")
  }
}

// TODO: TF it even means? RENAME!
fn relative_direction(direction: &Direction) -> (bool, i32) {
  let is_vert = vertical_workspaces();

  match direction {
      Direction::Up    =>  (is_vert, -1),
      Direction::Down  =>  (is_vert,  1),
      Direction::Left  => (!is_vert, -1),
      Direction::Right => (!is_vert,  1),
  }
}

fn determine_action(client: &ClientInfo, direction: Direction) -> Option<Action> {

  let (can_move, relative) = relative_direction(&direction);

  // TODO: Focusing a floating window from a tiled still does not work.
  if can_move && (client.floating || client.fullscreen) {
      return Some(Action::Move(relative));
  }

  // TODO: mixing one/two letter aliases with full names kinda sucks
  let x = client.x   as i64; // actual x + gap + border
  let y = client.y   as i64; // actual y + gap + border
  let w = client.width as i64;
  let h = client.height as i64;
  let mw = client.monitor.width as i64;
  let mh = client.monitor.height as i64;
  let decorations = get_decorations();

  let mut flag = match direction {
    // TODO: having bars will likely interfere as they reserve space.
    // TODO: adding the outer gap for `Down` and `Left` is not exactly correct
    // since the window may be surrounded by other windows and not "touch"
    // the screen border, but it may still work out as gaps aren't usually THAT large. 
    Direction::Down => y + h + decorations.outer_gaps + decorations.border_size - mh, 
    Direction::Up  => max(if y == decorations.outer_gaps + decorations.border_size { // < 0 if there's no bar
                0                                                                    // and dwindle:no_gaps_when_only is set
              } else {
                y - decorations.inner_gaps - decorations.border_size
              }, 0),
    Direction::Right => x + w + decorations.outer_gaps + decorations.border_size - mw, 
    Direction::Left => max(if x == decorations.outer_gaps + decorations.border_size { // same as `Up`
                0
              } else {
                x - decorations.inner_gaps - decorations.border_size
              }, 0),
  };

  if flag == decorations.outer_gaps + decorations.border_size { // `Down` and `Right` pseudofullscreen cases.
    flag = 0;                                                   // Not sure how to fix properly 🤷
  }

  if flag != 0 {
    Some(Action::RenameMe(direction))
  } else {
    if can_move {
      Some(Action::Move(relative))
    } else {
      None
    }
  }
}

fn do_stuff(action: Action, cmd: &str, is_only: bool) {
  let dir; // to store an otherwise temporary value. TODO: Looks like crap. Avoidable?
  let dispatcher = match action {
    Action::RenameMe(direction) => {
      dir = direction.to_string();
      Some(dispatch::DispatchType::Custom(cmd, &dir))
    },
    Action::Move(direction) => {
// TODO: separate logick for swap/moveto and focus/goto looks necessary
// TODO: or next workspace is occupied
//      if is_only && direction == 1 {
//        None
//      } else {
        match cmd {
          "swapwindow" => Some(dispatch::DispatchType::MoveToWorkspace(
            dispatch::WorkspaceIdentifierWithSpecial::Relative(direction), None)),
          "movefocus"  => Some(dispatch::DispatchType::Workspace(
            dispatch::WorkspaceIdentifierWithSpecial::Relative(direction))),
          //_ => panic!("Wrong dispatcher passed."),
          _ => None
        }
//      }
    },
  };
  if let Some(disp) = dispatcher {
    dispatch::Dispatch::call(disp).unwrap();
  }
}

fn main() -> hyprland::Result<()> {
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


  if let Some(active_window) = get_active_window() {
    if let Some(action) = determine_action(&active_window, direction) {
        do_stuff(action, &cmd, active_window.is_only);
    };
  } else {
    let (can_move, relative) = relative_direction(&direction);
    // TODO: allowing the previous workspace only is ~~most likely~~ incorrect
    // as long as it's possible to travel through workspaces by number.
    // Already: this check doesn't account for the ability to move the only window 
    // on the workspace to the next one
    if can_move && relative == -1 {
      do_stuff(Action::Move(relative), &cmd, true);
    }
  }

  Ok(())
}
