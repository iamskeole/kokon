// Settings → People (multi-user): named accounts, each with their own gallery (D-MU11).
// Shown only where a remote password is set and the viewer is not one of the named
// accounts; the shared password stays the operator's (Settings → Other devices). The
// account rules live in crates/qls-service/src/remote.rs and crates/qls-core/src/store.rs.

import { KeyRound, Trash2, UserPlus } from "lucide-react";
import { useEffect, useState } from "react";
import { ApiError, api } from "../lib/api";
import type { UserRecord } from "../lib/types";
import { native } from "../lib/native";
import { toastError, toastInfo } from "../lib/store";
import { Switch } from "./ui";

export function People() {
  // The server enforces the minimum (crates/qls-core/src/remote.rs MIN_PASSWORD_CHARS):
  // the buttons below disable under it, so the hint must say the same number.
  const MIN_PW = 8;
  const [users, setUsers] = useState<UserRecord[] | null>(null);
  const [multiUser, setMultiUser] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The create form and the per-row new-password field.
  const [newName, setNewName] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [creating, setCreating] = useState(false);
  const [resetId, setResetId] = useState<string | null>(null);
  const [resetPassword, setResetPassword] = useState("");
  const [busyRow, setBusyRow] = useState<string | null>(null);

  const load = () =>
    api
      .listUsers()
      .then((r) => {
        setUsers(r.users);
        setMultiUser(r.multi_user);
      })
      .catch((e) => setError(e instanceof ApiError ? e.body.message : "Kokon is not reachable."));

  useEffect(() => {
    load();
  }, []);

  const toggle = async (on: boolean) => {
    setMultiUser(on);
    try {
      await api.setMultiUser(on);
      toastInfo(
        on
          ? "Named accounts are on. People can sign in with their own name and password."
          : "Named accounts are off. People who were signed in sign in again with the shared password."
      );
    } catch (e) {
      setMultiUser(!on);
      toastError(e);
    }
  };

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    if (creating) return;
    setCreating(true);
    try {
      await api.createUser(newName, newPassword);
      setNewName("");
      setNewPassword("");
      await load();
    } catch (err) {
      toastError(err);
    } finally {
      setCreating(false);
    }
  };

  const reset = async (u: UserRecord) => {
    if (busyRow) return;
    setBusyRow(u.id);
    try {
      await api.setUserPassword(u.id, resetPassword);
      setResetId(null);
      setResetPassword("");
      toastInfo(`${u.name} signs in again with the new password.`);
    } catch (err) {
      toastError(err);
    } finally {
      setBusyRow(null);
    }
  };

  const remove = async (u: UserRecord) => {
    if (busyRow) return;
    if (!(await native.confirm(`Delete ${u.name}'s account? Their images and folders are removed. This cannot be undone.`, "Delete account"))) return;
    setBusyRow(u.id);
    try {
      const r = await api.deleteUser(u.id);
      toastInfo(`${u.name}'s account and ${r.rows_removed} ${r.rows_removed === 1 ? "image" : "images"} were removed.`);
      await load();
    } catch (err) {
      toastError(err);
    } finally {
      setBusyRow(null);
    }
  };

  if (error) return <p className="muted">{error}</p>;
  if (users === null) return <p className="muted">Loading…</p>;

  return (
    <>
      <div className="setting-card">
        <div className="setting-card-text">
          <strong>Named accounts</strong>
          <span className="muted">
            People you add sign in on any device with their own name and password and see only their own images. You keep the shared password.
          </span>
        </div>
        <Switch on={multiUser} onChange={toggle} label={multiUser ? "On" : "Off"} />
      </div>

      <form className="setting-card people-form" onSubmit={create}>
        <div className="setting-card-text">
          <strong>Add an account</strong>
          <span className="muted">
            The name is 1–20 characters and stays the same on every device; the password is {MIN_PW} or more characters. The person signs in with it and sees only their own images.
          </span>
        </div>
        <div className="people-row-fields">
          <input
            className="input"
            type="text"
            placeholder="Name"
            aria-label="Account name"
            style={{ flex: "1 1 150px" }}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            spellCheck={false}
          />
          <input
            className="input"
            type="password"
            placeholder={`Password (${MIN_PW}+ characters)`}
            aria-label="Account password"
            style={{ flex: "1 1 150px" }}
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
          />
          <button className="btn primary" type="submit" disabled={creating || !newName.trim() || newPassword.length < MIN_PW}>
            <UserPlus size={15} /> Add
          </button>
        </div>
      </form>

      {users.length === 0 ? (
        <p className="muted">No accounts yet.</p>
      ) : (
        users.map((u) => (
          <div className="setting-card people-row" key={u.id}>
            <div className="setting-card-text">
              <strong>{u.name}</strong>
              <span className="muted">added {new Date(u.created_at).toLocaleDateString()}</span>
            </div>
            <div className="row-btns">
              <button
                className="btn"
                onClick={() => {
                  setResetId(resetId === u.id ? null : u.id);
                  setResetPassword("");
                }}
              >
                <KeyRound size={14} /> New password
              </button>
              <button className="btn danger" disabled={busyRow === u.id} onClick={() => remove(u)}>
                <Trash2 size={14} /> Delete
              </button>
            </div>
          </div>
        ))
      )}
      {resetId !== null && (
        <div className="setting-card people-form">
          <div className="setting-card-text">
            <strong>New password for {users.find((u) => u.id === resetId)?.name}</strong>
            <span className="muted">At least {MIN_PW} characters. The person signs in again with it.</span>
          </div>
          <div className="people-row-fields">
            <input
              className="input"
              type="password"
              placeholder={`New password (${MIN_PW}+ characters)`}
              aria-label="New password"
              style={{ flex: "1 1 200px" }}
              value={resetPassword}
              onChange={(e) => setResetPassword(e.target.value)}
            />
            <button className="btn primary" disabled={busyRow === resetId || resetPassword.length < MIN_PW} onClick={() => reset(users.find((u) => u.id === resetId)!)}>
              Set
            </button>
          </div>
        </div>
      )}
      <p className="muted" style={{ fontSize: 12 }}>
        Deleting an account removes its images from the gallery and from disk. The shared password (Settings → Other devices) is unchanged.
      </p>
    </>
  );
}
