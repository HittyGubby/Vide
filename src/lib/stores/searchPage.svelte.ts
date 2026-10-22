import type { SearchResultItem } from "$lib/types";

class SearchPageState {
  query = $state("");
  activePlatform = $state("");
  results = $state<SearchResultItem[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  searched = $state(false);
  page = $state(1);
  urlMode = $state(false);
  readonly pageSize = 20;
}

export const searchPage = new SearchPageState();
