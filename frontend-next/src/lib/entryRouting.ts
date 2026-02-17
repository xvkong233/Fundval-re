export type HealthLike = {
  system_initialized?: boolean;
} & Record<string, any>;

export type EntryRoute = "/initialize" | "/dashboard" | "/login";

export const decideEntryRoute = (health: HealthLike, authed: boolean): EntryRoute => {
  if (health?.system_initialized === false) return "/initialize";
  if (health?.system_initialized === true && authed) return "/dashboard";
  return "/login";
};

export const shouldRedirectAuthedPublicPage = (authed: boolean): boolean => authed;

