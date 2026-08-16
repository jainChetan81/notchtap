import { isString } from "./guards";

export type ActionError = string | string[] | Error;

export function describeActionError(reason: ActionError): string {
  if (Array.isArray(reason)) return reason.map(String).join(", ");
  if (isString(reason)) return reason;
  return "Something went wrong";
}
