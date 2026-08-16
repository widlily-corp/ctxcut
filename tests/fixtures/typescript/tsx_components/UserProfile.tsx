import React, { useMemo } from 'react';
import { User } from '../simple_service/types';

export interface UserProfileProps {
    user: User;
    onUpdate: (updated: User) => void;
    className?: string;
}

export function UserProfile({ user, onUpdate, className }: UserProfileProps): JSX.Element {
    const formattedDate = useMemo(() => user.createdAt.toLocaleDateString(), [user.createdAt]);

    return (
        <div className={`user-card ${className ?? ''}`}>
            <h2>{user.email}</h2>
            <span>Role: {user.role}</span>
            <p>Member since: {formattedDate}</p>
            <button onClick={() => onUpdate(user)}>Edit Profile</button>
        </div>
    );
}
