import { useEffect, useState } from "react";
import { profileIndexStatus, type FinderDomain, type FolderProfileStatus } from "../lib/ipc";

/**
 * Whether a folder's index has its file profile, by one cheap lookup at launch and after each
 * finished pass. `needsAnalyse` is an index written before the profile existed: the card says so and
 * the user presses Analyse. Nothing is rebuilt by itself, and a failed lookup says nothing.
 */
export function useProfileStatus(
  domain: FinderDomain,
  folder: string | null,
  finishedPasses: number,
): FolderProfileStatus | null {
  const [status, setStatus] = useState<FolderProfileStatus | null>(null);

  useEffect(() => {
    if (folder === null) {
      setStatus(null);
      return;
    }
    let current = true;
    profileIndexStatus().then(
      (found) => {
        if (current) {
          setStatus(found[domain]);
        }
      },
      () => {
        if (current) {
          setStatus(null);
        }
      },
    );
    return () => {
      current = false;
    };
  }, [domain, folder, finishedPasses]);

  return status;
}
