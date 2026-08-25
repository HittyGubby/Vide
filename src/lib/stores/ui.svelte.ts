class UIStore {
  /** Whether the full-screen player detail overlay is shown. */
  showPlayerDetail = $state(false);
  /** Incremented each time sidebar search button is clicked, for TitleBar to react. */
  focusTitleBarSearch = $state(0);
}

export const ui = new UIStore();
