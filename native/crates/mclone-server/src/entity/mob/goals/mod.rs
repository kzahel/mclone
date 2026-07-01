#![allow(dead_code)]

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum GoalFlag {
    Move,
    Look,
    Jump,
    Target,
}

impl GoalFlag {
    const ALL: [Self; 4] = [Self::Move, Self::Look, Self::Jump, Self::Target];

    const fn bit(self) -> u8 {
        match self {
            Self::Move => 1 << 0,
            Self::Look => 1 << 1,
            Self::Jump => 1 << 2,
            Self::Target => 1 << 3,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Move => 0,
            Self::Look => 1,
            Self::Jump => 2,
            Self::Target => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GoalFlags(u8);

impl GoalFlags {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const MOVE: Self = Self(GoalFlag::Move.bit());
    pub(crate) const LOOK: Self = Self(GoalFlag::Look.bit());
    pub(crate) const JUMP: Self = Self(GoalFlag::Jump.bit());
    pub(crate) const TARGET: Self = Self(GoalFlag::Target.bit());

    pub(crate) fn of(flags: impl IntoIterator<Item = GoalFlag>) -> Self {
        let mut value = Self::NONE;
        for flag in flags {
            value.insert(flag);
        }
        value
    }

    pub(crate) const fn contains(self, flag: GoalFlag) -> bool {
        self.0 & flag.bit() != 0
    }

    pub(crate) const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub(crate) fn insert(&mut self, flag: GoalFlag) {
        self.0 |= flag.bit();
    }

    pub(crate) fn remove(&mut self, flag: GoalFlag) {
        self.0 &= !flag.bit();
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = GoalFlag> {
        GoalFlag::ALL
            .into_iter()
            .filter(move |flag| self.contains(*flag))
    }
}

pub(crate) trait Goal {
    fn can_use(&mut self) -> bool;

    fn can_continue_to_use(&mut self) -> bool {
        self.can_use()
    }

    fn is_interruptable(&self) -> bool {
        true
    }

    fn start(&mut self) {}

    fn stop(&mut self) {}

    fn tick(&mut self) {}

    fn flags(&self) -> GoalFlags {
        GoalFlags::NONE
    }

    fn set_flags(&mut self, _flags: GoalFlags) {}
}

struct WrappedGoal {
    priority: i32,
    goal: Box<dyn Goal>,
    is_running: bool,
}

impl WrappedGoal {
    fn new(priority: i32, goal: Box<dyn Goal>) -> Self {
        Self {
            priority,
            goal,
            is_running: false,
        }
    }

    fn can_be_replaced_by(&self, candidate_priority: i32) -> bool {
        self.goal.is_interruptable() && candidate_priority < self.priority
    }

    fn can_use(&mut self) -> bool {
        self.goal.can_use()
    }

    fn can_continue_to_use(&mut self) -> bool {
        self.goal.can_continue_to_use()
    }

    fn start(&mut self) {
        if !self.is_running {
            self.is_running = true;
            self.goal.start();
        }
    }

    fn stop(&mut self) {
        if self.is_running {
            self.is_running = false;
            self.goal.stop();
        }
    }

    fn tick(&mut self) {
        self.goal.tick();
    }

    fn is_running(&self) -> bool {
        self.is_running
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn flags(&self) -> GoalFlags {
        self.goal.flags()
    }
}

pub(crate) struct GoalSelector {
    locked_flags: [Option<usize>; 4],
    available_goals: Vec<WrappedGoal>,
    disabled_flags: GoalFlags,
    tick_count: i32,
    new_goal_rate: i32,
}

impl Default for GoalSelector {
    fn default() -> Self {
        Self {
            locked_flags: [None; 4],
            available_goals: Vec::new(),
            disabled_flags: GoalFlags::NONE,
            tick_count: 0,
            new_goal_rate: 3,
        }
    }
}

impl GoalSelector {
    pub(crate) fn add_goal(&mut self, priority: i32, goal: impl Goal + 'static) {
        self.available_goals
            .push(WrappedGoal::new(priority, Box::new(goal)));
    }

    pub(crate) fn remove_all_goals(&mut self) {
        self.available_goals.clear();
        self.locked_flags = [None; 4];
    }

    pub(crate) fn tick(&mut self) {
        self.cleanup_running_goals();
        self.clear_stopped_locked_flags();
        self.update_available_goals();
        self.tick_running_goals();
    }

    pub(crate) fn set_new_goal_rate(&mut self, new_goal_rate: i32) {
        self.new_goal_rate = new_goal_rate;
    }

    pub(crate) fn disable_control_flag(&mut self, flag: GoalFlag) {
        self.disabled_flags.insert(flag);
    }

    pub(crate) fn enable_control_flag(&mut self, flag: GoalFlag) {
        self.disabled_flags.remove(flag);
    }

    pub(crate) fn set_control_flag(&mut self, flag: GoalFlag, enabled: bool) {
        if enabled {
            self.enable_control_flag(flag);
        } else {
            self.disable_control_flag(flag);
        }
    }

    pub(crate) fn running_goal_count(&self) -> usize {
        self.available_goals
            .iter()
            .filter(|goal| goal.is_running())
            .count()
    }

    pub(crate) fn available_goal_count(&self) -> usize {
        self.available_goals.len()
    }

    fn cleanup_running_goals(&mut self) {
        for goal in &mut self.available_goals {
            if !goal.is_running() {
                continue;
            }
            if goal.flags().intersects(self.disabled_flags) || !goal.can_continue_to_use() {
                goal.stop();
            }
        }
    }

    fn clear_stopped_locked_flags(&mut self) {
        for locked in &mut self.locked_flags {
            if locked.is_some_and(|index| !self.available_goals[index].is_running()) {
                *locked = None;
            }
        }
    }

    fn update_available_goals(&mut self) {
        for candidate_index in 0..self.available_goals.len() {
            if self.available_goals[candidate_index].is_running() {
                continue;
            }
            let candidate_flags = self.available_goals[candidate_index].flags();
            if candidate_flags.intersects(self.disabled_flags) {
                continue;
            }
            if !self.can_use_locked_flags(candidate_index, candidate_flags) {
                continue;
            }
            if !self.available_goals[candidate_index].can_use() {
                continue;
            }

            for flag in candidate_flags.iter() {
                if let Some(locked_index) = self.locked_flags[flag.index()] {
                    self.available_goals[locked_index].stop();
                }
                self.locked_flags[flag.index()] = Some(candidate_index);
            }
            self.available_goals[candidate_index].start();
        }
    }

    fn can_use_locked_flags(&self, candidate_index: usize, candidate_flags: GoalFlags) -> bool {
        let candidate_priority = self.available_goals[candidate_index].priority();
        candidate_flags.iter().all(|flag| {
            self.locked_flags[flag.index()].is_none_or(|locked_index| {
                self.available_goals[locked_index].can_be_replaced_by(candidate_priority)
            })
        })
    }

    fn tick_running_goals(&mut self) {
        for goal in &mut self.available_goals {
            if goal.is_running() {
                goal.tick();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Event {
        CanUse(&'static str),
        CanContinue(&'static str),
        Start(&'static str),
        Stop(&'static str),
        Tick(&'static str),
    }

    #[derive(Clone, Default)]
    struct EventLog {
        events: Rc<RefCell<Vec<Event>>>,
    }

    impl EventLog {
        fn push(&self, event: Event) {
            self.events.borrow_mut().push(event);
        }

        fn take(&self) -> Vec<Event> {
            self.events.take()
        }
    }

    #[derive(Clone)]
    struct ProbeHandle {
        can_use: Rc<Cell<bool>>,
        can_continue: Rc<Cell<bool>>,
    }

    impl ProbeHandle {
        fn set_can_use(&self, value: bool) {
            self.can_use.set(value);
        }

        fn set_can_continue(&self, value: bool) {
            self.can_continue.set(value);
        }
    }

    struct ProbeGoal {
        name: &'static str,
        flags: GoalFlags,
        interruptable: bool,
        events: EventLog,
        handle: ProbeHandle,
    }

    impl ProbeGoal {
        fn new(name: &'static str, flags: GoalFlags, events: EventLog) -> (Self, ProbeHandle) {
            Self::with_interruptable(name, flags, true, events)
        }

        fn with_interruptable(
            name: &'static str,
            flags: GoalFlags,
            interruptable: bool,
            events: EventLog,
        ) -> (Self, ProbeHandle) {
            let handle = ProbeHandle {
                can_use: Rc::new(Cell::new(true)),
                can_continue: Rc::new(Cell::new(true)),
            };
            (
                Self {
                    name,
                    flags,
                    interruptable,
                    events,
                    handle: handle.clone(),
                },
                handle,
            )
        }
    }

    impl Goal for ProbeGoal {
        fn can_use(&mut self) -> bool {
            self.events.push(Event::CanUse(self.name));
            self.handle.can_use.get()
        }

        fn can_continue_to_use(&mut self) -> bool {
            self.events.push(Event::CanContinue(self.name));
            self.handle.can_continue.get()
        }

        fn is_interruptable(&self) -> bool {
            self.interruptable
        }

        fn start(&mut self) {
            self.events.push(Event::Start(self.name));
        }

        fn stop(&mut self) {
            self.events.push(Event::Stop(self.name));
        }

        fn tick(&mut self) {
            self.events.push(Event::Tick(self.name));
        }

        fn flags(&self) -> GoalFlags {
            self.flags
        }

        fn set_flags(&mut self, flags: GoalFlags) {
            self.flags = flags;
        }
    }

    #[test]
    fn lower_priority_number_replaces_running_interruptible_goal() {
        let mut selector = GoalSelector::default();
        let events = EventLog::default();
        let (old, _old_handle) = ProbeGoal::new("old", GoalFlags::MOVE, events.clone());
        let (new, new_handle) = ProbeGoal::new("new", GoalFlags::MOVE, events.clone());
        new_handle.set_can_use(false);
        selector.add_goal(5, old);
        selector.add_goal(1, new);
        selector.tick();
        events.take();
        new_handle.set_can_use(true);

        selector.tick();

        assert_eq!(selector.running_goal_count(), 1);
        assert_eq!(
            events.take(),
            vec![
                Event::CanContinue("old"),
                Event::CanUse("new"),
                Event::Stop("old"),
                Event::Start("new"),
                Event::Tick("new"),
            ]
        );
    }

    #[test]
    fn locked_flags_prevent_replacing_non_interruptible_goal() {
        let mut selector = GoalSelector::default();
        let events = EventLog::default();
        let (old, _old_handle) =
            ProbeGoal::with_interruptable("old", GoalFlags::MOVE, false, events.clone());
        let (new, new_handle) = ProbeGoal::new("new", GoalFlags::MOVE, events.clone());
        new_handle.set_can_use(false);
        selector.add_goal(5, old);
        selector.add_goal(1, new);
        selector.tick();
        events.take();
        new_handle.set_can_use(true);

        selector.tick();

        assert_eq!(selector.running_goal_count(), 1);
        assert_eq!(
            events.take(),
            vec![Event::CanContinue("old"), Event::Tick("old")]
        );
    }

    #[test]
    fn same_priority_cannot_replace_running_goal() {
        let mut selector = GoalSelector::default();
        let events = EventLog::default();
        let (old, _old_handle) = ProbeGoal::new("old", GoalFlags::LOOK, events.clone());
        let (new, new_handle) = ProbeGoal::new("new", GoalFlags::LOOK, events.clone());
        new_handle.set_can_use(false);
        selector.add_goal(5, old);
        selector.add_goal(5, new);
        selector.tick();
        events.take();
        new_handle.set_can_use(true);

        selector.tick();

        assert_eq!(
            events.take(),
            vec![Event::CanContinue("old"), Event::Tick("old")]
        );
    }

    #[test]
    fn cleanup_runs_before_update_and_goal_tick() {
        let mut selector = GoalSelector::default();
        let events = EventLog::default();
        let (old, old_handle) = ProbeGoal::new("old", GoalFlags::MOVE, events.clone());
        let (new, new_handle) = ProbeGoal::new("new", GoalFlags::MOVE, events.clone());
        new_handle.set_can_use(false);
        selector.add_goal(5, old);
        selector.add_goal(1, new);
        selector.tick();
        events.take();
        old_handle.set_can_continue(false);
        old_handle.set_can_use(false);
        new_handle.set_can_use(true);

        selector.tick();

        assert_eq!(
            events.take(),
            vec![
                Event::CanContinue("old"),
                Event::Stop("old"),
                Event::CanUse("old"),
                Event::CanUse("new"),
                Event::Start("new"),
                Event::Tick("new"),
            ]
        );
    }

    #[test]
    fn disabled_flags_stop_running_goals_and_prevent_new_starts() {
        let mut selector = GoalSelector::default();
        let events = EventLog::default();
        let (goal, _goal_handle) = ProbeGoal::new("move", GoalFlags::MOVE, events.clone());
        selector.add_goal(0, goal);
        selector.tick();
        events.take();

        selector.disable_control_flag(GoalFlag::Move);
        selector.tick();

        assert_eq!(selector.running_goal_count(), 0);
        assert_eq!(events.take(), vec![Event::Stop("move")]);
    }

    #[test]
    fn independent_flags_can_run_together() {
        let mut selector = GoalSelector::default();
        let events = EventLog::default();
        let (move_goal, _move_handle) = ProbeGoal::new("move", GoalFlags::MOVE, events.clone());
        let (look_goal, _look_handle) = ProbeGoal::new("look", GoalFlags::LOOK, events.clone());
        selector.add_goal(0, move_goal);
        selector.add_goal(0, look_goal);

        selector.tick();

        assert_eq!(selector.running_goal_count(), 2);
        assert_eq!(
            events.take(),
            vec![
                Event::CanUse("move"),
                Event::Start("move"),
                Event::CanUse("look"),
                Event::Start("look"),
                Event::Tick("move"),
                Event::Tick("look"),
            ]
        );
    }
}
