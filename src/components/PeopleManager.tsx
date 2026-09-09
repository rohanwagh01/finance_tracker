import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import { Button, TextField } from "./ui";

export default function PeopleManager() {
  const qc = useQueryClient();
  const people = useQuery({ queryKey: ["people"], queryFn: api.listPeople });
  const [name, setName] = useState("");

  const invalidate = () => qc.invalidateQueries({ queryKey: ["people"] });

  const create = useMutation({
    mutationFn: () => api.createPerson({ name }),
    onSuccess: () => {
      setName("");
      invalidate();
    },
  });
  const rename = useMutation({
    mutationFn: (v: { id: string; name: string }) =>
      api.updatePerson(v.id, { name: v.name }),
    onSuccess: invalidate,
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deletePerson(id),
    onSuccess: invalidate,
  });

  return (
    <div className="space-y-3">
      <ul className="space-y-2">
        {people.data?.map((p) => (
          <li key={p.id} className="flex items-center gap-2">
            <input
              defaultValue={p.name}
              disabled={p.is_self}
              onBlur={(e) => {
                const v = e.target.value.trim();
                if (v && v !== p.name) rename.mutate({ id: p.id, name: v });
              }}
              className="flex-1 rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)] disabled:opacity-60"
            />
            {p.is_self ? (
              <span className="text-xs text-[var(--muted)]">you</span>
            ) : (
              <Button variant="ghost" onClick={() => remove.mutate(p.id)}>
                Remove
              </Button>
            )}
          </li>
        ))}
      </ul>

      <div className="flex items-end gap-2">
        <TextField
          label="Add a person"
          placeholder="e.g. Partner, Roommate"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="flex-1"
        />
        <Button
          variant="ghost"
          disabled={!name.trim() || create.isPending}
          onClick={() => create.mutate()}
        >
          Add
        </Button>
      </div>

      {(create.isError || rename.isError || remove.isError) && (
        <p className="text-sm text-red-500">
          {errorMessage(create.error ?? rename.error ?? remove.error)}
        </p>
      )}
    </div>
  );
}
