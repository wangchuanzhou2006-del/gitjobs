-- Returns a verified user by email.
create or replace function get_user_by_email(p_email text)
returns table(
    user_id uuid,
    auth_hash bytea,
    email text,
    email_verified boolean,
    has_password boolean,
    has_profile boolean,
    moderator boolean,
    name text,
    username text,
    password text
) as $$
    select
        u.user_id,
        u.auth_hash,
        u.email,
        u.email_verified,
        u.password is not null as has_password,
        p.job_seeker_profile_id is not null as has_profile,
        u.moderator,
        u.name,
        u.username,
        null::text as password
    from "user" u
    left join job_seeker_profile p on u.user_id = p.user_id
    where u.email = p_email
    and u.email_verified = true;
$$ language sql;

-- Returns a verified user by id.
create or replace function get_user_by_id_verified(p_user_id uuid)
returns table(
    user_id uuid,
    auth_hash bytea,
    email text,
    email_verified boolean,
    has_password boolean,
    has_profile boolean,
    moderator boolean,
    name text,
    username text,
    password text
) as $$
    select
        u.user_id,
        u.auth_hash,
        u.email,
        u.email_verified,
        u.password is not null as has_password,
        p.job_seeker_profile_id is not null as has_profile,
        u.moderator,
        u.name,
        u.username,
        null::text as password
    from "user" u
    left join job_seeker_profile p on u.user_id = p.user_id
    where u.user_id = p_user_id
    and u.email_verified = true;
$$ language sql;

-- Returns a verified user by username including password.
create or replace function get_user_by_username(p_username text)
returns table(
    user_id uuid,
    auth_hash bytea,
    email text,
    email_verified boolean,
    has_password boolean,
    has_profile boolean,
    moderator boolean,
    name text,
    username text,
    password text
) as $$
    select
        u.user_id,
        u.auth_hash,
        u.email,
        u.email_verified,
        u.password is not null as has_password,
        p.job_seeker_profile_id is not null as has_profile,
        u.moderator,
        u.name,
        u.username,
        u.password
    from "user" u
    left join job_seeker_profile p on u.user_id = p.user_id
    where u.username = p_username
    and u.password is not null
    and u.email_verified = true;
$$ language sql;

-- Returns the password hash for the provided user id.
create or replace function get_user_password(p_user_id uuid)
returns text as $$
    select u.password
    from "user" u
    where u.user_id = p_user_id;
$$ language sql;

-- Checks whether the image is publicly visible.
create or replace function is_image_public(p_image_id uuid)
returns boolean as $$
    select exists (
        select 1
        from employer e
        join job j using (employer_id)
        where e.logo_id = p_image_id
        and j.first_published_at is not null
    );
$$ language sql;

-- Creates a user and generates an email verification code if needed.
create or replace function sign_up_user(
    p_user_summary jsonb,
    p_email_verified boolean
)
returns table(
    user_id uuid,
    auth_hash bytea,
    email text,
    email_verified boolean,
    has_password boolean,
    has_profile boolean,
    moderator boolean,
    name text,
    username text,
    verification_code uuid
) as $$
declare
    v_verification_code uuid;
begin
    -- Insert the new user and fill the return payload
    insert into "user" (
        auth_hash,
        email,
        email_verified,
        name,
        password,
        username
    ) values (
        gen_random_bytes(32),
        p_user_summary->>'email',
        p_email_verified,
        p_user_summary->>'name',
        p_user_summary->>'password',
        p_user_summary->>'username'
    )
    returning
        "user".user_id,
        "user".auth_hash,
        "user".email,
        "user".email_verified,
        "user".password is not null,
        "user".moderator,
        "user".name,
        "user".username
    into
        user_id,
        auth_hash,
        email,
        email_verified,
        has_password,
        moderator,
        name,
        username;

    -- New users do not have profiles yet
    has_profile := false;

    -- If the email is not verified, generate an email verification code
    if not p_email_verified then
        insert into email_verification_code (user_id)
        values (user_id)
        returning email_verification_code_id into v_verification_code;
    end if;

    -- Return the verification code when generated
    verification_code := v_verification_code;

    return next;
end
$$ language plpgsql;

-- Updates user details from the provided payload.
create or replace function update_user_details(
    p_user_id uuid,
    p_user_summary jsonb
)
returns void as $$
    update "user" set
        name = p_user_summary->>'name'
    where user_id = p_user_id;
$$ language sql;

-- Updates user password and rotates auth hash.
create or replace function update_user_password(
    p_user_id uuid,
    p_new_password text
)
returns void as $$
    update "user" set
        auth_hash = gen_random_bytes(32),
        password = p_new_password
    where user_id = p_user_id;
$$ language sql;

-- Check if the user has access to the image provided.
create or replace function user_has_image_access(p_user_id uuid, p_image_id uuid)
returns boolean as $$
begin
    -- User is a moderator
    perform from "user"
    where user_id = p_user_id
    and moderator = true;
    if found then return true; end if;

    -- Profile photo or employer logo: user created the image
    perform from image
    where image_id = p_image_id
    and created_by = p_user_id;
    if found then return true; end if;

    -- Profile photo: applied to a employer's job
    perform from job_seeker_profile p
    join application a on p.job_seeker_profile_id = a.job_seeker_profile_id
    join job j on a.job_id = j.job_id
    join employer_team et on j.employer_id = et.employer_id
    where p.photo_id = p_image_id
    and et.user_id = p_user_id
    and et.approved = true;
    if found then return true; end if;

    -- Employer logo: user belongs to the employer team
    perform from employer e
    join employer_team et using (employer_id)
    where e.logo_id = p_image_id
    and et.user_id = p_user_id
    and et.approved = true;
    return found;
end
$$ language plpgsql;

-- Checks whether the user can access the provided job seeker profile.
create or replace function user_has_profile_access(
    p_user_id uuid,
    p_job_seeker_profile_id uuid
)
returns boolean as $$
    select exists (
        select 1
        from job_seeker_profile p
        join application a on p.job_seeker_profile_id = a.job_seeker_profile_id
        join job j on a.job_id = j.job_id
        join employer_team et on j.employer_id = et.employer_id
        where et.user_id = p_user_id
        and p.job_seeker_profile_id = p_job_seeker_profile_id
        and et.approved = true
    );
$$ language sql;

-- Checks whether the user belongs to the employer team as an approved member.
create or replace function user_owns_employer(p_user_id uuid, p_employer_id uuid)
returns boolean as $$
    select exists (
        select 1
        from employer_team
        where user_id = p_user_id
        and employer_id = p_employer_id
        and approved = true
    );
$$ language sql;

-- Checks whether the user belongs to the employer team that owns the job.
create or replace function user_owns_job(p_user_id uuid, p_job_id uuid)
returns boolean as $$
    select exists (
        select 1
        from job j
        join employer_team et using (employer_id)
        where et.user_id = p_user_id
        and j.job_id = p_job_id
        and et.approved = true
    );
$$ language sql;

-- Verifies email using a non-expired verification code.
create or replace function verify_email(p_code uuid)
returns void as $$
declare
    v_user_id uuid;
begin
    -- Delete the verification code if it's valid and return the associated user_id
    delete from email_verification_code
    where email_verification_code_id = p_code
    and created_at > current_timestamp - interval '1 day'
    returning user_id into v_user_id;

    -- If no user_id is returned, the code is invalid or expired
    if v_user_id is null then
        raise exception 'email verification failed: invalid or expired code';
    end if;

    -- Mark the user's email as verified
    update "user"
    set email_verified = true
    where user_id = v_user_id;
end
$$ language plpgsql;

-- Accepts a pending team member invitation.
create or replace function accept_team_member_invitation(
    p_employer_id uuid,
    p_user_id uuid
)
returns void as $$
    update employer_team
    set approved = true
    where employer_id = p_employer_id
    and user_id = p_user_id;
$$ language sql;

-- Adds a new employer with memberships and an initial approved team member.
create or replace function add_employer(
    p_user_id uuid,
    p_employer jsonb
)
returns uuid as $$
declare
    v_employer_id uuid;
begin
    -- Insert employer data from payload
    insert into employer (
        company,
        description,
        public,
        location_id,
        logo_id,
        website_url
    ) values (
        p_employer->>'company',
        p_employer->>'description',
        (p_employer->>'public')::boolean,
        ((p_employer->'location')->>'location_id')::uuid,
        (p_employer->>'logo_id')::uuid,
        p_employer->>'website_url'
    )
    returning employer_id into v_employer_id;

    -- Insert employer memberships from payload
    insert into employer_member (employer_id, member_id)
    select
        v_employer_id,
        (member->>'member_id')::uuid
    from jsonb_array_elements(
        case
            when p_employer ? 'members' and jsonb_typeof(p_employer->'members') <> 'null' then
                p_employer->'members'
            else '[]'::jsonb
        end
    ) member
    on conflict (employer_id, member_id) do nothing;

    -- Add creator as an approved team member
    insert into employer_team (employer_id, user_id, approved)
    values (v_employer_id, p_user_id, true);

    return v_employer_id;
end
$$ language plpgsql;

-- Adds a job and its projects/certifications for an employer.
create or replace function add_job(p_employer_id uuid, p_job jsonb)
returns void as $$
declare
    v_job_id uuid;
begin
    -- Insert job base data
    insert into job (
        employer_id,
        kind,
        status,
        location_id,
        workplace,
        title,
        description,
        apply_instructions,
        apply_url,
        benefits,
        open_source,
        qualifications,
        responsibilities,
        salary,
        salary_usd_year,
        salary_currency,
        salary_min,
        salary_min_usd_year,
        salary_max,
        salary_max_usd_year,
        salary_period,
        seniority,
        skills,
        tz_end,
        tz_start,
        upstream_commitment
    ) values (
        p_employer_id,
        p_job->>'kind',
        p_job->>'status',
        ((p_job->'location')->>'location_id')::uuid,
        p_job->>'workplace',
        p_job->>'title',
        p_job->>'description',
        p_job->>'apply_instructions',
        p_job->>'apply_url',
        (
            case
                when p_job ? 'benefits' and jsonb_typeof(p_job->'benefits') <> 'null' then
                    array(select jsonb_array_elements_text(p_job->'benefits'))
                else null
            end
        ),
        (p_job->>'open_source')::int,
        p_job->>'qualifications',
        p_job->>'responsibilities',
        (p_job->>'salary')::bigint,
        (p_job->>'salary_usd_year')::bigint,
        p_job->>'salary_currency',
        (p_job->>'salary_min')::bigint,
        (p_job->>'salary_min_usd_year')::bigint,
        (p_job->>'salary_max')::bigint,
        (p_job->>'salary_max_usd_year')::bigint,
        p_job->>'salary_period',
        p_job->>'seniority',
        (
            case
                when p_job ? 'skills' and jsonb_typeof(p_job->'skills') <> 'null' then
                    array(select jsonb_array_elements_text(p_job->'skills'))
                else null
            end
        ),
        p_job->>'tz_end',
        p_job->>'tz_start',
        (p_job->>'upstream_commitment')::int
    )
    returning job_id into v_job_id;

    -- Insert related projects
    insert into job_project (job_id, project_id)
    select
        v_job_id,
        (project->>'project_id')::uuid
    from jsonb_array_elements(
        case
            when p_job ? 'projects' and jsonb_typeof(p_job->'projects') <> 'null' then
                p_job->'projects'
            else '[]'::jsonb
        end
    ) project;

    -- Insert required certifications
    insert into job_certification (job_id, certification_id)
    select
        v_job_id,
        (certification->>'certification_id')::uuid
    from jsonb_array_elements(
        case
            when p_job ? 'certifications' and jsonb_typeof(p_job->'certifications') <> 'null' then
                p_job->'certifications'
            else '[]'::jsonb
        end
    ) certification;
end
$$ language plpgsql;

-- Adds a team member invitation by user email.
create or replace function add_team_member(
    p_employer_id uuid,
    p_email text
)
returns uuid as $$
    insert into employer_team (
        employer_id,
        user_id,
        approved
    )
    select
        p_employer_id,
        user_id,
        false
    from "user"
    where email = p_email
    on conflict do nothing
    returning user_id;
$$ language sql;

-- Approves a job and returns the previous first_published_at value.
create or replace function approve_job(p_job_id uuid, p_reviewer uuid)
returns timestamptz as $$
    with old as (
        select first_published_at
        from job
        where job_id = p_job_id
    )
    update job
    set
        status = 'published',
        first_published_at = coalesce(first_published_at, current_timestamp),
        published_at = current_timestamp,
        reviewed_at = current_timestamp,
        reviewed_by = p_reviewer
    where job_id = p_job_id
    returning (select first_published_at from old);
$$ language sql;

-- Archives a pending or published job.
create or replace function archive_job(p_job_id uuid)
returns void as $$
    update job
    set
        status = 'archived',
        archived_at = current_timestamp,
        updated_at = current_timestamp
    where job_id = p_job_id
    and (status = 'pending-approval' or status = 'published');
$$ language sql;

-- Cancels a job application for the given user.
create or replace function cancel_application(
    p_application_id uuid,
    p_user_id uuid
)
returns void as $$
    delete from application
    where application_id in (
        select a.application_id
        from application a
        join job_seeker_profile p using (job_seeker_profile_id)
        where a.application_id = p_application_id
        and p.user_id = p_user_id
    );
$$ language sql;

-- Soft-deletes a job.
create or replace function delete_job(p_job_id uuid)
returns void as $$
    update job
    set
        status = 'deleted',
        deleted_at = current_timestamp
    where job_id = p_job_id
    and status <> 'deleted';
$$ language sql;

-- Deletes a team member while preserving at least one approved member.
create or replace function delete_team_member(
    p_employer_id uuid,
    p_user_id uuid
)
returns void as $$
declare
    v_approved boolean;
begin
    -- Check the target membership state
    select approved
    into v_approved
    from employer_team
    where employer_id = p_employer_id
    and user_id = p_user_id;

    if not found then
        raise exception 'team member not found';
    end if;

    -- Approved members can only be removed if another approved member remains
    if v_approved then
        delete from employer_team
        where employer_id = p_employer_id
        and user_id = p_user_id
        and (
            select count(*)
            from employer_team
            where employer_id = p_employer_id
            and approved = true
        ) > 1;

        if not found then
            raise exception 'cannot remove last approved team member';
        end if;
    else
        -- Pending invitations can be deleted directly
        delete from employer_team
        where employer_id = p_employer_id
        and user_id = p_user_id;
    end if;
end
$$ language plpgsql;

-- Returns filter options for employer applications.
create or replace function get_applications_filters_options(p_employer_id uuid)
returns json as $$
    select json_build_object(
        'jobs',
        coalesce(json_agg(json_build_object(
            'job_id', j.job_id,
            'created_at', j.created_at,
            'title', j.title,
            'status', j.status,
            'workplace', j.workplace,
            'city', l.city,
            'country', l.country
        ) order by j.created_at desc), '[]'::json)
    )
    from job j
    left join location l using (location_id)
    where j.employer_id = p_employer_id
    and j.status <> 'deleted';
$$ language sql;

-- Returns employer details.
create or replace function get_employer(p_employer_id uuid)
returns json as $$
    select nullif(jsonb_strip_nulls(jsonb_build_object(
        'company', e.company,
        'description', e.description,
        'public', e.public,
        'logo_id', e.logo_id,
        'website_url', e.website_url,
        'location', nullif(jsonb_strip_nulls(jsonb_build_object(
            'location_id', l.location_id,
            'city', l.city,
            'country', l.country,
            'state', l.state
        )), '{}'::jsonb),
        'members', (
            select json_agg(json_build_object(
                'member_id', m.member_id,
                'foundation', m.foundation,
                'level', m.level,
                'logo_url', m.logo_url,
                'name', m.name
            ) order by m.foundation asc, m.name asc)
            from employer_member em
            join member m on em.member_id = m.member_id
            where em.employer_id = e.employer_id
        )
    )), '{}'::jsonb)
    from employer e
    left join location l using (location_id)
    where e.employer_id = p_employer_id;
$$ language sql;

-- Returns full job details for employer dashboard.
create or replace function get_job_dashboard(p_job_id uuid)
returns json as $$
    select nullif(jsonb_strip_nulls(jsonb_build_object(
        'description', j.description,
        'status', j.status,
        'title', j.title,
        'kind', j.kind,
        'workplace', j.workplace,
        'apply_instructions', j.apply_instructions,
        'apply_url', j.apply_url,
        'benefits', j.benefits,
        'job_id', j.job_id,
        'location', nullif(jsonb_strip_nulls(jsonb_build_object(
            'location_id', l.location_id,
            'city', l.city,
            'country', l.country,
            'state', l.state
        )), '{}'::jsonb),
        'open_source', j.open_source,
        'projects', (
            select json_agg(json_build_object(
                'project_id', p.project_id,
                'foundation', p.foundation,
                'logo_url', p.logo_url,
                'maturity', p.maturity,
                'name', p.name
            ))
            from project p
            join job_project jp on p.project_id = jp.project_id
            where jp.job_id = j.job_id
        ),
        'published_at', j.published_at,
        'qualifications', j.qualifications,
        'responsibilities', j.responsibilities,
        'review_notes', j.review_notes,
        'salary', j.salary,
        'salary_currency', j.salary_currency,
        'salary_min', j.salary_min,
        'salary_max', j.salary_max,
        'salary_period', j.salary_period,
        'seniority', j.seniority,
        'skills', j.skills,
        'certifications', (
            select json_agg(json_build_object(
                'certification_id', c.certification_id,
                'name', c.name,
                'provider', c.provider,
                'short_name', c.short_name,
                'description', c.description,
                'url', c.url,
                'logo_url', c.logo_url
            ))
            from certification c
            join job_certification jc on c.certification_id = jc.certification_id
            where jc.job_id = j.job_id
        ),
        'tz_end', j.tz_end,
        'tz_start', j.tz_start,
        'updated_at', j.updated_at,
        'upstream_commitment', j.upstream_commitment
    )), '{}'::jsonb)
    from job j
    left join location l using (location_id)
    where j.job_id = p_job_id
    and j.status <> 'deleted';
$$ language sql;

-- Returns salary fields required to refresh normalized salary values.
create or replace function get_job_salary(p_job_id uuid)
returns table(
    salary bigint,
    salary_currency text,
    salary_min bigint,
    salary_max bigint,
    salary_period text
) as $$
    select
        j.salary,
        j.salary_currency,
        j.salary_min,
        j.salary_max,
        j.salary_period
    from job j
    where j.job_id = p_job_id;
$$ language sql;

-- Returns the job seeker profile for the provided user.
create or replace function get_job_seeker_profile(p_user_id uuid)
returns json as $$
    select nullif(jsonb_strip_nulls(jsonb_build_object(
        'bluesky_url', p.bluesky_url,
        'certifications', p.certifications,
        'education', p.education,
        'email', p.email,
        'experience', p.experience,
        'facebook_url', p.facebook_url,
        'github_url', p.github_url,
        'linkedin_url', p.linkedin_url,
        'location', nullif(jsonb_strip_nulls(jsonb_build_object(
            'city', l.city,
            'country', l.country,
            'location_id', l.location_id,
            'state', l.state
        )), '{}'::jsonb),
        'name', p.name,
        'open_to_relocation', p.open_to_relocation,
        'open_to_remote', p.open_to_remote,
        'phone', p.phone,
        'photo_id', p.photo_id,
        'projects', p.projects,
        'public', p.public,
        'skills', p.skills,
        'summary', p.summary,
        'twitter_url', p.twitter_url,
        'website_url', p.website_url
    )), '{}'::jsonb)
    from job_seeker_profile p
    left join location l using (location_id)
    where p.user_id = p_user_id;
$$ language sql;

-- Returns the owner user id for the provided job seeker profile.
create or replace function get_job_seeker_user_id(p_job_seeker_profile_id uuid)
returns uuid as $$
    select p.user_id
    from job_seeker_profile p
    where p.job_seeker_profile_id = p_job_seeker_profile_id;
$$ language sql;

-- Returns stats for a specific job in json format.
create or replace function get_job_stats(p_job_id uuid)
returns json as $$
    select json_strip_nulls(json_build_object(
        'search_appearances_daily', (
            select coalesce(json_agg(json_build_array(
                floor(extract(epoch from day) * 1000),
                total
            )), '[]'::json)
            from (
                select day, total
                from search_appearances
                where job_id = p_job_id
                and day >= current_date - '1 month'::interval
                order by day asc
            ) daily_search_appearances
        ),
        'search_appearances_total_last_month', (
            select coalesce(sum(total), 0)
            from search_appearances
            where job_id = p_job_id
            and day >= current_date - '1 month'::interval
        ),
        'views_daily', (
            select coalesce(json_agg(json_build_array(
                floor(extract(epoch from day) * 1000),
                total
            )), '[]'::json)
            from (
                select day, total
                from job_views
                where job_id = p_job_id
                and day >= current_date - '1 month'::interval
                order by day asc
            ) daily_views
        ),
        'views_total_last_month', (
            select coalesce(sum(total), 0)
            from job_views
            where job_id = p_job_id
            and day >= current_date - '1 month'::interval
        )
    ));
$$ language sql;

-- Returns the number of pending team invitations for the user.
create or replace function get_user_invitations_count(p_user_id uuid)
returns bigint as $$
    select count(*)
    from employer_team et
    where et.user_id = p_user_id
    and et.approved = false;
$$ language sql;

-- Returns available certifications.
create or replace function list_certifications()
returns json as $$
    select coalesce(json_agg(json_build_object(
        'certification_id', c.certification_id,
        'name', c.name,
        'provider', c.provider,
        'short_name', c.short_name,
        'description', c.description,
        'logo_url', c.logo_url,
        'url', c.url
    ) order by c.name asc), '[]'::json)
    from certification c;
$$ language sql;

-- Returns all non-deleted jobs for an employer.
create or replace function list_employer_jobs(p_employer_id uuid)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'job_id', j.job_id,
        'created_at', j.created_at,
        'title', j.title,
        'status', j.status,
        'workplace', j.workplace,
        'archived_at', j.archived_at,
        'published_at', j.published_at,
        'review_notes', j.review_notes,
        'city', l.city,
        'country', l.country
    ) order by j.published_at desc, j.created_at desc), '[]'::json)
    from job j
    left join location l using (location_id)
    where j.employer_id = p_employer_id
    and j.status <> 'deleted';
$$ language sql;

-- Returns employers where the user is an approved team member.
create or replace function list_employers(p_user_id uuid)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'employer_id', e.employer_id,
        'company', e.company,
        'logo_id', e.logo_id
    ) order by e.company asc), '[]'::json)
    from employer e
    join employer_team et using (employer_id)
    where et.user_id = p_user_id
    and et.approved = true;
$$ language sql;

-- Returns available foundations.
create or replace function list_foundations()
returns json as $$
    select coalesce(json_agg(json_build_object(
        'name', f.name
    ) order by f.name asc), '[]'::json)
    from foundation f;
$$ language sql;

-- Returns all job applications for the provided user.
create or replace function list_job_seeker_applications(p_user_id uuid)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'application_id', a.application_id,
        'applied_at', a.applied_at,
        'job_id', a.job_id,
        'job_location', a.job_location,
        'job_status', a.job_status,
        'job_title', a.job_title,
        'job_workplace', a.job_workplace
    ) order by a.applied_at desc), '[]'::json)
    from (
        select
            ap.application_id,
            ap.created_at as applied_at,
            ap.job_id,
            nullif(jsonb_strip_nulls(jsonb_build_object(
                'city', l.city,
                'country', l.country,
                'location_id', l.location_id,
                'state', l.state
            )), '{}'::jsonb) as job_location,
            j.status as job_status,
            j.title as job_title,
            j.workplace as job_workplace
        from application ap
        join job j on ap.job_id = j.job_id
        join job_seeker_profile p on ap.job_seeker_profile_id = p.job_seeker_profile_id
        left join location l on j.location_id = l.location_id
        where p.user_id = p_user_id
    ) a;
$$ language sql;

-- Returns jobs for moderation filtered by status.
create or replace function list_jobs_for_moderation(p_status text)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'created_at', j.created_at,
        'employer', jsonb_strip_nulls(jsonb_build_object(
            'company', e.company,
            'employer_id', e.employer_id,
            'logo_id', e.logo_id,
            'members', members.members,
            'website_url', e.website_url
        )),
        'job_id', j.job_id,
        'title', j.title
    ) order by j.created_at desc), '[]'::json)
    from job j
    join employer e on j.employer_id = e.employer_id
    left join lateral (
        select jsonb_agg(jsonb_build_object(
            'foundation', m.foundation,
            'level', m.level,
            'logo_url', m.logo_url,
            'member_id', m.member_id,
            'name', m.name
        ) order by m.foundation asc, m.name asc) as members
        from employer_member em
        join member m on em.member_id = m.member_id
        where em.employer_id = e.employer_id
    ) members on true
    where j.status = p_status;
$$ language sql;

-- Returns team members for an employer.
create or replace function list_team_members(p_employer_id uuid)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'approved', et.approved,
        'email', u.email,
        'name', u.name,
        'user_id', u.user_id,
        'username', u.username
    ) order by u.name asc), '[]'::json)
    from employer_team et
    join "user" u using (user_id)
    where et.employer_id = p_employer_id;
$$ language sql;

-- Returns pending team invitations for a user.
create or replace function list_user_invitations(p_user_id uuid)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'company', e.company,
        'created_at', e.created_at,
        'employer_id', e.employer_id
    ) order by e.created_at desc), '[]'::json)
    from employer_team et
    join employer e using (employer_id)
    where et.user_id = p_user_id
    and et.approved = false;
$$ language sql;

-- Publishes a job and refreshes normalized salary values.
create or replace function publish_job(
    p_job_id uuid,
    p_salary_usd_year bigint,
    p_salary_min_usd_year bigint,
    p_salary_max_usd_year bigint
)
returns void as $$
    update job
    set
        status = 'pending-approval',
        updated_at = current_timestamp,
        archived_at = null,
        salary_usd_year = p_salary_usd_year,
        salary_min_usd_year = p_salary_min_usd_year,
        salary_max_usd_year = p_salary_max_usd_year
    where job_id = p_job_id
    and (status = 'archived' or status = 'draft' or status = 'rejected');
$$ language sql;

-- Rejects a pending or published job and updates review metadata.
create or replace function reject_job(
    p_job_id uuid,
    p_reviewer uuid,
    p_review_notes text
)
returns void as $$
    update job
    set
        status = 'rejected',
        review_notes = p_review_notes,
        reviewed_at = current_timestamp,
        reviewed_by = p_reviewer
    where job_id = p_job_id
    and (status = 'pending-approval' or status = 'published');
$$ language sql;

-- Returns the applications that match the filters provided.
create or replace function search_applications(
    p_employer_id uuid,
    p_filters jsonb
)
returns json as $$
declare
    v_job_id uuid := (p_filters->>'job_id')::uuid;
    v_limit int := coalesce((p_filters->>'limit')::int, 20);
    v_offset int := coalesce((p_filters->>'offset')::int, 0);
begin
    return (
    with filtered_applications as (
        select
            a.application_id,
            a.created_at as applied_at,
            j.job_id,
            j.title as job_title,
            (
                select nullif(jsonb_strip_nulls(jsonb_build_object(
                    'location_id', l.location_id,
                    'city', l.city,
                    'country', l.country,
                    'state', l.state
                )), '{}'::jsonb)
            ) as job_location,
            j.workplace as job_workplace,
            p.job_seeker_profile_id,
            p.photo_id,
            p.name,
            (
                select format(
                    '%s at %s', experience->>'title', experience->>'company'
                ) as last_position
                from (
                    select jsonb_array_elements(p.experience) as experience
                )
                order by (experience->>'end_date')::date desc nulls first
                limit 1
            ) as last_position
        from application a
        join job j on a.job_id = j.job_id
        join job_seeker_profile p on a.job_seeker_profile_id = p.job_seeker_profile_id
        left join location l on j.location_id = l.location_id
        where j.employer_id = p_employer_id
        and j.status <> 'deleted'
        and
            case when v_job_id is not null then
            a.job_id = v_job_id else true end
    )
    select json_build_object(
        'applications',
        (
            select coalesce(json_agg(json_build_object(
                'application_id', application_id,
                'applied_at', applied_at,
                'job_id', job_id,
                'job_title', job_title,
                'job_location', job_location,
                'job_seeker_profile_id', job_seeker_profile_id,
                'job_workplace', job_workplace,
                'photo_id', photo_id,
                'name', name,
                'last_position', last_position
            )), '[]'::json)
            from (
                select *
                from filtered_applications
                order by applied_at desc
                limit v_limit
                offset v_offset
            ) filtered_applications_page
        ),
        'total',
        (
            select count(*) from filtered_applications
        )
    )
    );
end
$$ language plpgsql;

-- Updates employer details and refreshes its memberships.
create or replace function update_employer(
    p_employer_id uuid,
    p_employer jsonb
)
returns void as $$
begin
    -- Update employer core fields
    update employer
    set
        company = p_employer->>'company',
        description = p_employer->>'description',
        public = (p_employer->>'public')::boolean,
        location_id = ((p_employer->'location')->>'location_id')::uuid,
        logo_id = (p_employer->>'logo_id')::uuid,
        website_url = p_employer->>'website_url',
        updated_at = current_timestamp
    where employer_id = p_employer_id;

    -- Replace memberships with payload values
    delete from employer_member
    where employer_id = p_employer_id;

    insert into employer_member (employer_id, member_id)
    select
        p_employer_id,
        (member->>'member_id')::uuid
    from jsonb_array_elements(
        case
            when p_employer ? 'members' and jsonb_typeof(p_employer->'members') <> 'null' then
                p_employer->'members'
            else '[]'::jsonb
        end
    ) member
    on conflict (employer_id, member_id) do nothing;
end
$$ language plpgsql;

-- Updates a non-deleted job and refreshes its projects/certifications.
create or replace function update_job(p_job_id uuid, p_job jsonb)
returns void as $$
begin
    -- Update job fields from payload
    update job
    set
        kind = p_job->>'kind',
        status = p_job->>'status',
        location_id = ((p_job->'location')->>'location_id')::uuid,
        workplace = p_job->>'workplace',
        title = p_job->>'title',
        description = p_job->>'description',
        apply_instructions = p_job->>'apply_instructions',
        apply_url = p_job->>'apply_url',
        benefits = (
            case
                when p_job ? 'benefits' and jsonb_typeof(p_job->'benefits') <> 'null' then
                    array(select jsonb_array_elements_text(p_job->'benefits'))
                else null
            end
        ),
        open_source = (p_job->>'open_source')::int,
        qualifications = p_job->>'qualifications',
        responsibilities = p_job->>'responsibilities',
        salary = (p_job->>'salary')::bigint,
        salary_usd_year = (p_job->>'salary_usd_year')::bigint,
        salary_currency = p_job->>'salary_currency',
        salary_min = (p_job->>'salary_min')::bigint,
        salary_min_usd_year = (p_job->>'salary_min_usd_year')::bigint,
        salary_max = (p_job->>'salary_max')::bigint,
        salary_max_usd_year = (p_job->>'salary_max_usd_year')::bigint,
        salary_period = p_job->>'salary_period',
        seniority = p_job->>'seniority',
        skills = (
            case
                when p_job ? 'skills' and jsonb_typeof(p_job->'skills') <> 'null' then
                    array(select jsonb_array_elements_text(p_job->'skills'))
                else null
            end
        ),
        tz_end = p_job->>'tz_end',
        tz_start = p_job->>'tz_start',
        upstream_commitment = (p_job->>'upstream_commitment')::int,
        updated_at = current_timestamp
    where job_id = p_job_id
    and status <> 'deleted';

    if found then
        -- Replace projects with payload values
        delete from job_project
        where job_id = p_job_id;

        insert into job_project (job_id, project_id)
        select
            p_job_id,
            (project->>'project_id')::uuid
        from jsonb_array_elements(
            case
                when p_job ? 'projects' and jsonb_typeof(p_job->'projects') <> 'null' then
                    p_job->'projects'
                else '[]'::jsonb
            end
        ) project;

        -- Replace certifications with payload values
        delete from job_certification
        where job_id = p_job_id;

        insert into job_certification (job_id, certification_id)
        select
            p_job_id,
            (certification->>'certification_id')::uuid
        from jsonb_array_elements(
            case
                when p_job ? 'certifications' and jsonb_typeof(p_job->'certifications') <> 'null' then
                    p_job->'certifications'
                else '[]'::jsonb
            end
        ) certification;
    end if;
end
$$ language plpgsql;

-- Upserts the job seeker profile for the provided user.
create or replace function upsert_job_seeker_profile(p_user_id uuid, p_profile jsonb)
returns void as $$
begin
    insert into job_seeker_profile (
        user_id,
        email,
        name,
        public,
        summary,
        bluesky_url,
        certifications,
        education,
        experience,
        facebook_url,
        github_url,
        linkedin_url,
        location_id,
        open_to_relocation,
        open_to_remote,
        phone,
        photo_id,
        projects,
        skills,
        twitter_url,
        website_url
    ) values (
        p_user_id,
        p_profile->>'email',
        p_profile->>'name',
        (p_profile->>'public')::boolean,
        p_profile->>'summary',
        p_profile->>'bluesky_url',
        nullif(p_profile->'certifications', 'null'::jsonb),
        nullif(p_profile->'education', 'null'::jsonb),
        nullif(p_profile->'experience', 'null'::jsonb),
        p_profile->>'facebook_url',
        p_profile->>'github_url',
        p_profile->>'linkedin_url',
        ((p_profile->'location')->>'location_id')::uuid,
        (p_profile->>'open_to_relocation')::boolean,
        (p_profile->>'open_to_remote')::boolean,
        p_profile->>'phone',
        (p_profile->>'photo_id')::uuid,
        nullif(p_profile->'projects', 'null'::jsonb),
        (
            case
                when p_profile ? 'skills' then
                    array(select jsonb_array_elements_text(p_profile->'skills'))
                else null
            end
        ),
        p_profile->>'twitter_url',
        p_profile->>'website_url'
    )
    on conflict (user_id) do update set
        email = excluded.email,
        name = excluded.name,
        public = excluded.public,
        summary = excluded.summary,
        bluesky_url = excluded.bluesky_url,
        certifications = excluded.certifications,
        education = excluded.education,
        experience = excluded.experience,
        facebook_url = excluded.facebook_url,
        github_url = excluded.github_url,
        linkedin_url = excluded.linkedin_url,
        location_id = excluded.location_id,
        open_to_relocation = excluded.open_to_relocation,
        open_to_remote = excluded.open_to_remote,
        phone = excluded.phone,
        photo_id = excluded.photo_id,
        projects = excluded.projects,
        skills = excluded.skills,
        twitter_url = excluded.twitter_url,
        website_url = excluded.website_url;
end
$$ language plpgsql;

-- Returns an image version. We'll try first to get the version of the size
-- requested. If it doesn't exist, we'll return the svg version (if available).
create or replace function get_image_version(p_image_id uuid, p_version text)
returns table(data bytea, format text) as $$
begin
    -- PNG
    return query select iv.data, 'png' as format from image_version iv
    where image_id = p_image_id and version = p_version;
    if found then return; end if;

    -- SVG
    return query select iv.data, 'svg' as format from image_version iv
    where image_id = p_image_id and version = 'svg';
end
$$ language plpgsql;

-- Applies the user profile to a published job.
create or replace function apply_to_job(p_job_id uuid, p_user_id uuid)
returns boolean as $$
    with inserted_application as (
        insert into application (
            job_id,
            job_seeker_profile_id
        )
        select
            j.job_id,
            p.job_seeker_profile_id
        from job j
        join job_seeker_profile p on p.user_id = p_user_id
        where j.job_id = p_job_id
        and j.status = 'published'
        on conflict (job_seeker_profile_id, job_id) do nothing
        returning application_id
    )
    select exists (select 1 from inserted_application);
$$ language sql;

-- Returns the full published job payload for the job board.
create or replace function get_job_jobboard(p_job_id uuid)
returns json as $$
    select nullif(jsonb_strip_nulls(jsonb_build_object(
        'description', j.description,
        'job_id', j.job_id,
        'kind', j.kind,
        'title', j.title,
        'workplace', j.workplace,
        'apply_instructions', j.apply_instructions,
        'apply_url', j.apply_url,
        'benefits', j.benefits,
        'location', nullif(jsonb_strip_nulls(jsonb_build_object(
            'location_id', l.location_id,
            'city', l.city,
            'country', l.country,
            'state', l.state
        )), '{}'::jsonb),
        'open_source', j.open_source,
        'projects', (
            select json_agg(json_build_object(
                'project_id', p.project_id,
                'foundation', p.foundation,
                'logo_url', p.logo_url,
                'maturity', p.maturity,
                'name', p.name
            ))
            from project p
            join job_project jp on p.project_id = jp.project_id
            where jp.job_id = j.job_id
        ),
        'published_at', j.published_at,
        'qualifications', j.qualifications,
        'responsibilities', j.responsibilities,
        'salary', j.salary,
        'salary_currency', j.salary_currency,
        'salary_min', j.salary_min,
        'salary_max', j.salary_max,
        'salary_period', j.salary_period,
        'seniority', j.seniority,
        'skills', j.skills,
        'certifications', (
            select json_agg(json_build_object(
                'certification_id', c.certification_id,
                'name', c.name,
                'provider', c.provider,
                'short_name', c.short_name,
                'description', c.description,
                'logo_url', c.logo_url,
                'url', c.url
            ))
            from certification c
            join job_certification jc on c.certification_id = jc.certification_id
            where jc.job_id = j.job_id
        ),
        'tz_end', j.tz_end,
        'tz_start', j.tz_start,
        'updated_at', j.updated_at,
        'upstream_commitment', j.upstream_commitment,
        'employer', nullif(jsonb_strip_nulls(jsonb_build_object(
            'company', e.company,
            'description', e.description,
            'employer_id', e.employer_id,
            'logo_id', e.logo_id,
            'members', members.members,
            'website_url', e.website_url
        )), '{}'::jsonb)
    )), '{}'::jsonb)
    from job j
    join employer e on j.employer_id = e.employer_id
    left join lateral (
        select
            jsonb_agg(jsonb_build_object(
                'member_id', m.member_id,
                'foundation', m.foundation,
                'level', m.level,
                'logo_url', m.logo_url,
                'name', m.name
            ) order by m.foundation asc, m.name asc) as members
        from employer_member em
        join member m on em.member_id = m.member_id
        where em.employer_id = e.employer_id
    ) members on true
    left join location l on j.location_id = l.location_id
    where j.job_id = p_job_id
    and j.status = 'published';
$$ language sql;

-- Returns available filters options for job board searches.
create or replace function get_jobs_filters_options()
returns json as $$
    select json_build_object(
        'foundations',
        coalesce(json_agg(json_build_object(
            'name', f.name
        ) order by f.name asc), '[]'::json)
    )
    from foundation f;
$$ language sql;

-- Returns some stats about the job board in json format.
create or replace function get_stats()
returns json as $$
    select json_strip_nulls(json_build_object(
        'jobs', json_build_object(
            'published_per_foundation', (
                select json_agg(json_build_array(foundation, jobs))
                from (
                    select foundation, count(*) as jobs
                    from (
                        select distinct f.name as foundation, j.job_id
                        from job j join job_project jp on j.job_id = jp.job_id
                        join project p on jp.project_id = p.project_id
                        join foundation f on p.foundation = f.name
                        where j.first_published_at is not null
                    )
                    group by foundation
                    order by jobs desc
                ) foundation_jobs
            ),
            'published_per_month', (
                select json_agg(json_build_array(year, month, total))
                from (
                    select
                        to_char(first_published_at, 'YYYY') as year,
                        to_char(first_published_at, 'Mon') as month,
                        count(*) as total
                    from job
                    where first_published_at is not null
                    group by
                        to_char(first_published_at, 'YYYY'),
                        to_char(first_published_at, 'Mon')
                ) year_month_count
            ),
            'published_running_total', (
                select json_agg(json_build_array(
                    floor(extract(epoch from jobs_day) * 1000),
                    running_total
                ))
                from (
                    select
                        jobs_day,
                        sum(total) over (order by jobs_day asc) as running_total
                    from (
                        select
                            date_trunc('day', first_published_at) as jobs_day,
                            count(*) as total
                        from job
                        where first_published_at is not null
                        group by date_trunc('day', first_published_at)
                    ) mt
                ) rt
            ),
            'views_daily', (
                select json_agg(json_build_array(
                    floor(extract(epoch from day) * 1000),
                    total
                ))
                from (
                    select day, sum(total) as total
                    from job_views
                    where day >= current_date - '1 month'::interval
                    group by day
                    order by day asc
                ) dt
            ),
            'views_monthly', (
                select json_agg(json_build_array(
                    floor(extract(epoch from month) * 1000),
                    total
                ))
                from (
                    select date_trunc('month', day) as month, sum(total) as total
                    from job_views
                    where day >= current_date - '2 year'::interval
                    group by month
                    order by month asc
                ) mt
            )
        ),
        'ts_now', floor(extract(epoch from current_timestamp) * 1000),
        'ts_one_month_ago', floor(extract(epoch from current_timestamp - '1 month'::interval) * 1000),
        'ts_two_years_ago', floor(extract(epoch from current_timestamp - '2 year'::interval) * 1000)
    ));
$$ language sql;

-- Returns the jobs that match the filters provided.
create or replace function search_jobs(p_filters jsonb)
returns json as $$
declare
    v_benefits text[];
    v_date_from date;
    v_date_to date;
    v_foundation text := (p_filters->>'foundation');
    v_kind text[];
    v_limit int := coalesce((p_filters->>'limit')::int, 20);
    v_location_id uuid := ((p_filters->'location')->>'location_id')::uuid;
    v_max_distance real := (p_filters->>'max_distance')::real;
    v_membership text := (p_filters->>'membership');
    v_offset int := coalesce((p_filters->>'offset')::int, 0);
    v_open_source int := (p_filters->>'open_source')::int;
    v_salary_min bigint := (p_filters->>'salary_min')::bigint;
    v_seniority text := (p_filters->>'seniority');
    v_skills text[];
    v_sort text := coalesce((p_filters->>'sort'), 'date');
    v_tsquery_with_prefix_matching tsquery;
    v_upstream_commitment int := (p_filters->>'upstream_commitment')::int;
    v_workplace text[];
begin
    -- Prepare filters
    if p_filters ? 'benefits' then
        select array_agg(e::text) into v_benefits
        from jsonb_array_elements_text(p_filters->'benefits') e;
    end if;
    if p_filters ? 'date_range' then
        v_date_to := current_date;
        case
            when (p_filters->>'date_range') = 'last-day' then
                v_date_from := current_date - interval '1 day';
            when (p_filters->>'date_range') = 'last3-days' then
                v_date_from := current_date - interval '3 days';
            when (p_filters->>'date_range') = 'last7-days' then
                v_date_from := current_date - interval '7 days';
            when (p_filters->>'date_range') = 'last30-days' then
                v_date_from := current_date - interval '30 days';
        end case;
    end if;
    if p_filters ? 'kind' then
        select array_agg(e::text) into v_kind
        from jsonb_array_elements_text(p_filters->'kind') e;
    end if;
    if p_filters ? 'skills' then
        select array_agg(e::text) into v_skills
        from jsonb_array_elements_text(p_filters->'skills') e;
    end if;
    if p_filters ? 'workplace' then
        select array_agg(e::text) into v_workplace
        from jsonb_array_elements_text(p_filters->'workplace') e;
    end if;
    if p_filters ? 'ts_query' then
        select ts_rewrite(
            websearch_to_tsquery(p_filters->>'ts_query'),
            format('
                select
                    to_tsquery(lexeme),
                    to_tsquery(lexeme || '':*'')
                from unnest(tsvector_to_array(to_tsvector(%L))) as lexeme
                ', p_filters->>'ts_query'
            )
        ) into v_tsquery_with_prefix_matching;
    end if;

    return (
    with filtered_jobs as (
        select
            j.job_id,
            j.kind,
            j.published_at,
            j.title,
            j.workplace,
            j.open_source,
            j.salary,
            j.salary_currency,
            j.salary_min,
            j.salary_max,
            j.salary_max_usd_year,
            j.salary_period,
            j.seniority,
            j.skills,
            j.updated_at,
            j.upstream_commitment,
            (
                select nullif(jsonb_strip_nulls(jsonb_build_object(
                    'company', e.company,
                    'employer_id', e.employer_id,
                    'logo_id', e.logo_id,
                    'members', members.members,
                    'website_url', e.website_url
                )), '{}'::jsonb)
            ) as employer,
            (
                select nullif(jsonb_strip_nulls(jsonb_build_object(
                    'location_id', l.location_id,
                    'city', l.city,
                    'country', l.country,
                    'state', l.state
                )), '{}'::jsonb)
            ) as location,
            (
                select json_agg(json_build_object(
                    'project_id', p.project_id,
                    'foundation', p.foundation,
                    'logo_url', p.logo_url,
                    'maturity', p.maturity,
                    'name', p.name
                ))
                from project p
                left join job_project using (project_id)
                left join job using (job_id)
                where job_id = j.job_id
            ) as projects
        from job j
        join employer e on j.employer_id = e.employer_id
        left join lateral (
            select
                jsonb_agg(jsonb_build_object(
                    'member_id', m.member_id,
                    'foundation', m.foundation,
                    'level', m.level,
                    'logo_url', m.logo_url,
                    'name', m.name
                ) order by m.foundation asc, m.name asc) as members
            from employer_member em
            join member m on em.member_id = m.member_id
            where em.employer_id = e.employer_id
        ) members on true
        left join location l on j.location_id = l.location_id
        where j.status = 'published'
        and
            case when cardinality(v_benefits) > 0 then
                j.benefits @> v_benefits
            else true end
        and
            case when v_date_from is not null and v_date_to is not null then
                j.published_at::date >= v_date_from and j.published_at::date <= v_date_to
            else true end
        and
            case when v_foundation is not null then
                j.job_id = any(
                    select job_id from job_project
                    where project_id = any(
                        select project_id from project
                        where foundation = v_foundation
                    )
                )
            else true end
        and
            case when cardinality(v_kind) > 0 then
                j.kind = any(v_kind)
            else true end
        and
            case when v_location_id is not null and v_max_distance is not null then
                st_dwithin(
                    (select coordinates from location where location_id = v_location_id),
                    (select coordinates from location where location_id = j.location_id),
                    v_max_distance
                )
            else true end
        and
            case when v_membership is not null then
                exists (
                    select 1
                    from employer_member em
                    join member m on em.member_id = m.member_id
                    where em.employer_id = e.employer_id
                    and m.foundation = v_membership
                )
            else true end
        and
            case when v_open_source is not null then
                j.open_source >= v_open_source
            else true end
        and
            case when p_filters ? 'projects' then
                j.job_id = any(
                    select job_id from job_project
                    where project_id = any(
                        select project_id
                        from project p
                        join jsonb_to_recordset(p_filters->'projects') as fp(foundation text, name text)
                        on p.foundation = fp.foundation and p.name = fp.name
                    )
                )
            else true end
        and
            case when v_salary_min is not null then
                j.salary_min_usd_year >= v_salary_min
            else true end
        and
            case when v_seniority is not null then
                j.seniority = v_seniority
            else true end
        and
            case when cardinality(v_skills) > 0 then
                j.skills @> v_skills
            else true end
        and
            case when v_tsquery_with_prefix_matching is not null then
                v_tsquery_with_prefix_matching @@ j.tsdoc
            else true end
        and
            case when v_upstream_commitment is not null then
                j.upstream_commitment >= v_upstream_commitment
            else true end
        and
            case when cardinality(v_workplace) > 0 then
                j.workplace = any(v_workplace)
            else true end
    )
    select json_build_object(
        'jobs',
        (
            select coalesce(json_agg(json_build_object(
                'job_id', job_id,
                'kind', kind,
                'published_at', published_at,
                'title', title,
                'workplace', workplace,
                'open_source', open_source,
                'salary', salary,
                'salary_currency', salary_currency,
                'salary_min', salary_min,
                'salary_max', salary_max,
                'salary_period', salary_period,
                'seniority', seniority,
                'skills', skills,
                'updated_at', updated_at,
                'upstream_commitment', upstream_commitment,
                'employer', employer,
                'location', location,
                'projects', projects
            )), '[]'::json)
            from (
                select *
                from filtered_jobs
                order by
                    (case when v_sort = 'open-source' then open_source end) desc nulls last,
                    (case when v_sort = 'salary' then salary_max_usd_year end) desc nulls last,
                    (case when v_sort = 'upstream-commitment' then upstream_commitment end) desc nulls last,
                    published_at desc
                limit v_limit
                offset v_offset
            ) filtered_jobs_page
        ),
        'total',
        (
            select count(*) from filtered_jobs
        )
    )
    );
end
$$ language plpgsql;

-- update_jobs_views updates the views of the jobs provided.
create or replace function update_jobs_views(p_lock_key bigint, p_data jsonb)
returns void as $$
    -- Make sure only one batch of updates is processed at a time
    select pg_advisory_xact_lock(p_lock_key);

    -- Insert or update the corresponding views counters as needed
    insert into job_views (job_id, day, total)
    select views_batch.*
    from (
        select
            (value->>0)::uuid as job_id,
            (value->>1)::date as day,
            (value->>2)::integer as total
        from jsonb_array_elements(p_data)
    ) as views_batch
    join job on job.job_id = views_batch.job_id
    where job.status = 'published'
    on conflict (job_id, day) do
    update set total = job_views.total + excluded.total;
$$ language sql;

-- update_search_appearances updates the search appearances of the jobs provided.
create or replace function update_search_appearances(p_lock_key bigint, p_data jsonb)
returns void as $$
    -- Make sure only one batch of updates is processed at a time
    select pg_advisory_xact_lock(p_lock_key);

    -- Insert or update the corresponding search appearances counters as needed
    insert into search_appearances (job_id, day, total)
    select appearances_batch.*
    from (
        select
            (value->>0)::uuid as job_id,
            (value->>1)::date as day,
            (value->>2)::integer as total
        from jsonb_array_elements(p_data)
    ) as appearances_batch
    join job on job.job_id = appearances_batch.job_id
    where job.status = 'published'
    on conflict (job_id, day) do
    update set total = search_appearances.total + excluded.total;
$$ language sql;

-- Returns the locations that match the query provided.
create or replace function search_locations(p_ts_query text)
returns table(
    location_id uuid,
    city text,
    country text,
    state text
) as $$
declare
    v_ts_query_with_prefix_matching tsquery;
begin
    -- Prepare ts query with prefix matching
    select ts_rewrite(
        websearch_to_tsquery(p_ts_query),
        format(
            '
            select
                to_tsquery(lexeme),
                to_tsquery(lexeme || '':*'')
            from unnest(tsvector_to_array(to_tsvector(%L))) as lexeme
            ', p_ts_query
        )
    ) into v_ts_query_with_prefix_matching;

    return query
    select
        lws.location_id,
        lws.city,
        lws.country,
        lws.state
    from (
        select
            l.location_id,
            l.city,
            l.country,
            l.state,
            ts_rank(tsdoc, v_ts_query_with_prefix_matching, 1) as score
        from location l
        where v_ts_query_with_prefix_matching @@ tsdoc
        order by score desc
        limit 20
    ) as lws;
end
$$ language plpgsql;

-- Returns matching locations as a json array.
create or replace function search_locations_json(p_ts_query text)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'location_id', l.location_id,
        'city', l.city,
        'country', l.country,
        'state', l.state
    )), '[]'::json)
    from search_locations(p_ts_query) l;
$$ language sql;

-- Returns members that match the provided foundation and member query.
create or replace function search_members(p_foundation text, p_member text)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'foundation', m.foundation,
        'level', m.level,
        'logo_url', m.logo_url,
        'member_id', m.member_id,
        'name', m.name
    ) order by m.name asc), '[]'::json)
    from (
        select
            foundation,
            level,
            logo_url,
            member_id,
            name
        from member
        where foundation = p_foundation
        and name ilike '%' || p_member || '%'
        order by name asc
        limit 20
    ) m;
$$ language sql;

-- Returns projects that match the provided foundation and project query.
create or replace function search_projects(p_foundation text, p_project text)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'foundation', p.foundation,
        'logo_url', p.logo_url,
        'maturity', p.maturity,
        'name', p.name,
        'project_id', p.project_id
    ) order by p.name asc), '[]'::json)
    from (
        select
            foundation,
            logo_url,
            maturity,
            name,
            project_id
        from project
        where foundation = p_foundation
        and name ilike '%' || p_project || '%'
        order by name asc
        limit 20
    ) p;
$$ language sql;

-- Enqueues a notification for delivery.
create or replace function enqueue_notification(
    p_kind text,
    p_template_data jsonb,
    p_recipients uuid[]
)
returns void as $$
declare
    v_notification_template_data_id uuid;
    v_template_hash text;
begin
    -- Insert or reuse template data and get its ID
    if p_template_data is not null then
        v_template_hash := encode(digest(convert_to(p_template_data::text, 'utf8'), 'sha256'), 'hex');

        insert into notification_template_data (data, hash)
        values (p_template_data, v_template_hash)
        on conflict (hash) do update set hash = notification_template_data.hash
        returning notification_template_data_id into v_notification_template_data_id;
    end if;

    -- Insert one notification per recipient
    insert into notification (kind, notification_template_data_id, user_id)
    select p_kind, v_notification_template_data_id, unnest(p_recipients);
end;
$$ language plpgsql;

-- Returns the next pending notification and locks it for processing.
create or replace function get_pending_notification()
returns table(
    email text,
    kind text,
    notification_id uuid,

    template_data jsonb
) as $$
    select
        u.email,
        n.kind,
        n.notification_id,

        ntd.data as template_data
    from notification n
    join "user" u using (user_id)
    left join notification_template_data ntd using (notification_template_data_id)
    where n.processed = false
    and (u.email_verified = true or n.kind = 'email-verification')
    order by n.created_at asc
    limit 1
    for update of n skip locked;
$$ language sql;

-- Marks a notification as processed and stores the delivery error, if any.
create or replace function update_notification(
    p_notification_id uuid,
    p_error text
)
returns void as $$
    update notification
    set
        processed = true,
        processed_at = current_timestamp,
        error = p_error
    where notification_id = p_notification_id;
$$ language sql;

-- Adds a member for the provided foundation.
create or replace function add_member(
    p_foundation text,
    p_name text,
    p_level text,
    p_logo_url text
)
returns void as $$
    insert into member (foundation, name, level, logo_url)
    values (p_foundation, p_name, p_level, p_logo_url);
$$ language sql;

-- Adds a project for the provided foundation.
create or replace function add_project(
    p_foundation text,
    p_name text,
    p_maturity text,
    p_logo_url text
)
returns void as $$
    insert into project (foundation, name, maturity, logo_url)
    values (p_foundation, p_name, p_maturity, p_logo_url);
$$ language sql;

-- Lists all foundations with a configured landscape URL.
-- Uses a syncer-specific name to avoid colliding with dashboard list_foundations().
create or replace function syncer_list_foundations()
returns json as $$
    select coalesce(json_agg(json_build_object(
        'landscape_url', f.landscape_url,
        'name', f.name
    ) order by f.name asc), '[]'::json)
    from foundation f
    where f.landscape_url is not null;
$$ language sql;

-- Lists all members for the provided foundation.
create or replace function list_members(p_foundation text)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'foundation', p_foundation,
        'level', m.level,
        'logo_url', m.logo_url,
        'name', m.name
    ) order by m.name asc), '[]'::json)
    from member m
    where m.foundation = p_foundation;
$$ language sql;

-- Lists all projects for the provided foundation.
create or replace function list_projects(p_foundation text)
returns json as $$
    select coalesce(json_agg(json_build_object(
        'foundation', p_foundation,
        'logo_url', p.logo_url,
        'maturity', p.maturity,
        'name', p.name
    ) order by p.name asc), '[]'::json)
    from project p
    where p.foundation = p_foundation;
$$ language sql;

-- Removes a member for the provided foundation.
create or replace function remove_member(p_foundation text, p_name text)
returns void as $$
    delete from member
    where foundation = p_foundation
    and name = p_name;
$$ language sql;

-- Removes a project for the provided foundation.
create or replace function remove_project(p_foundation text, p_name text)
returns void as $$
    delete from project
    where foundation = p_foundation
    and name = p_name;
$$ language sql;

-- Updates a member for the provided foundation.
create or replace function update_member(
    p_foundation text,
    p_name text,
    p_level text,
    p_logo_url text
)
returns void as $$
    update member
    set
        level = p_level,
        logo_url = p_logo_url
    where foundation = p_foundation
    and name = p_name;
$$ language sql;

-- Updates a project for the provided foundation.
create or replace function update_project(
    p_foundation text,
    p_name text,
    p_maturity text,
    p_logo_url text
)
returns void as $$
    update project
    set
        maturity = p_maturity,
        logo_url = p_logo_url
    where foundation = p_foundation
    and name = p_name;
$$ language sql;

-- Claims one queued item without blocking another worker's claim.
create or replace function claim_talent_import_item()
returns jsonb as $$
declare
    v_item jsonb;
    v_extracting_lease constant interval := interval '2 minutes';
begin
    with next_item as (
        select item_id
        from talent_import_items
        where status = 'queued'
           or (
               status = 'extracting'
               and updated_at < current_timestamp - v_extracting_lease
           )
        order by
            case when status = 'queued' then 0 else 1 end,
            created_at,
            batch_id,
            position
        for update skip locked
        limit 1
    )
    update talent_import_items i
    set status = 'extracting', updated_at = current_timestamp
    from next_item n
    where i.item_id = n.item_id
    returning row_to_json(i) into v_item;

    if v_item is not null then
        update talent_import_batches
        set status = 'processing'
        where batch_id = (v_item->>'batch_id')::uuid
          and status = 'queued';
    end if;

    return v_item;
end;
$$ language plpgsql;

create or replace function claim_talent_source_subscription()
returns jsonb as $$
declare
    v_claim jsonb;
begin
    with due as (
        select subscription_id
        from talent_source_subscriptions
        where enabled
          and next_run_at <= current_timestamp
          and (claimed_at is null or claimed_at < current_timestamp - interval '15 minutes')
        order by next_run_at, subscription_id
        for update skip locked
        limit 1
    ), claimed as (
        update talent_source_subscriptions subscription
        set claimed_at = current_timestamp, updated_at = current_timestamp
        from due
        where subscription.subscription_id = due.subscription_id
        returning subscription.*
    ), run as (
        insert into talent_source_runs (subscription_id)
        select subscription_id from claimed
        returning run_id, subscription_id
    )
    select jsonb_build_object(
        'run_id', run.run_id,
        'subscription', to_jsonb(claimed) || jsonb_build_object('source_name', source.name)
    )
    into v_claim
    from claimed
    join run using (subscription_id)
    join talent_sources source on source.source_id = claimed.source_id;
    return v_claim;
end;
$$ language plpgsql;

-- Removes expired encrypted raw material while preserving safe and audit data.
create or replace function cleanup_talent_raw_data(p_now timestamptz)
returns jsonb as $$
declare
    v_batches integer;
    v_items integer;
begin
    with due as (
        select batch_id
        from talent_import_batches
        where raw_deleted_at is null
          and (
              raw_delete_after <= p_now
              or created_at <= p_now - interval '90 days'
          )
        for update
    ),
    cleaned_items as (
        update talent_import_items i
        set
            raw_fragment_ciphertext = null,
            contact_candidates_ciphertext = null,
            updated_at = current_timestamp
        from due
        where i.batch_id = due.batch_id
          and (
              i.raw_fragment_ciphertext is not null
              or i.contact_candidates_ciphertext is not null
          )
        returning i.item_id
    )
    select count(*) into v_items from cleaned_items;

    update talent_import_batches
    set
        raw_text_ciphertext = null,
        raw_deleted_at = p_now
    where raw_deleted_at is null
      and (
          raw_delete_after <= p_now
          or created_at <= p_now - interval '90 days'
      );
    get diagnostics v_batches = row_count;

    return jsonb_build_object(
        'batches_cleaned', v_batches,
        'items_cleaned', v_items,
        'cleaned_at', p_now
    );
end;
$$ language plpgsql;

-- Completes extraction and advances batch counters in the same transaction.
create or replace function complete_talent_import_item(p_completion jsonb)
returns void as $$
declare
    v_batch_id uuid;
begin
    if p_completion is null or jsonb_typeof(p_completion) <> 'object' then
        raise exception 'talent import completion must be a JSON object'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(p_completion) as fields(field)
        where field <> all(
            array[
                'item_id',
                'status',
                'extraction',
                'extraction_confidence',
                'contact_candidates_ciphertext',
                'contact_hashes',
                'contact_masked',
                'normalized_fingerprint',
                'validation_errors',
                'risk_score',
                'risk_level',
                'risk_reasons',
                'duplicate_candidates'
            ]
        )
    ) then
        raise exception 'talent import completion contains an unknown field'
            using errcode = '22023';
    end if;
    if not (p_completion ? 'item_id')
       or jsonb_typeof(p_completion->'item_id') <> 'string'
       or coalesce(p_completion->>'item_id', '')
           !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then
        raise exception 'invalid talent import completion item ID' using errcode = '22023';
    end if;
    if p_completion ? 'status'
       and (
           jsonb_typeof(p_completion->'status') <> 'string'
           or p_completion->>'status' <> 'needs_review'
       ) then
        raise exception 'talent import completion can only require review' using errcode = '22023';
    end if;
    if jsonb_typeof(coalesce(p_completion->'extraction', 'null'::jsonb)) <> 'object' then
        raise exception 'talent extraction must be a JSON object' using errcode = '22023';
    end if;
    if p_completion->'extraction' ? 'contacts' then
        raise exception 'complete contacts are forbidden in safe talent extraction'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(p_completion->'extraction') as fields(field)
        where field <> all(
            array[
                'kind',
                'confidence',
                'title',
                'scene',
                'region',
                'city',
                'district',
                'salaryMin',
                'salaryMax',
                'roles',
                'skills',
                'experienceYears',
                'hoursPerDay',
                'restDaysPerMonth',
                'lodging',
                'housingAllowance',
                'availability',
                'urgency',
                'publishedAt',
                'missingFields',
                'riskCandidates'
            ]
        )
    ) then
        raise exception 'talent extraction contains an unknown field' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(p_completion->'extraction') as fields(field, value)
        where case
            when field = 'kind' then
                jsonb_typeof(value) <> 'string'
                or value #>> '{}' not in ('hiring', 'seeking', 'irrelevant', 'uncertain')
            when field = 'confidence' then
                jsonb_typeof(value) <> 'number'
                or case
                    when jsonb_typeof(value) = 'number'
                        then (value #>> '{}')::numeric not between 0 and 1
                    else false
                end
            when field in (
                'title', 'scene', 'region', 'city', 'district',
                'lodging', 'availability', 'urgency', 'publishedAt'
            ) then jsonb_typeof(value) not in ('string', 'null')
            when field in (
                'salaryMin', 'salaryMax', 'experienceYears', 'hoursPerDay',
                'restDaysPerMonth', 'housingAllowance'
            ) then jsonb_typeof(value) not in ('number', 'null')
            when field in ('roles', 'skills', 'missingFields', 'riskCandidates')
                then jsonb_typeof(value) <> 'array'
            else true
        end
    ) then
        raise exception 'talent extraction contains an invalid field type'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(p_completion->'extraction') as arrays(field, value)
        cross join lateral jsonb_array_elements(
            case when jsonb_typeof(value) = 'array' then value else '[]'::jsonb end
        ) as entries(entry)
        where field in ('roles', 'skills', 'missingFields')
          and (
              jsonb_typeof(entry) <> 'string'
              or coalesce(entry #>> '{}', '') = ''
          )
    ) then
        raise exception 'talent extraction contains an invalid array entry'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'extraction'->'riskCandidates') = 'array'
                    then p_completion->'extraction'->'riskCandidates'
                else '[]'::jsonb
            end
        ) as risks(risk)
        where jsonb_typeof(risk) <> 'object'
           or not (risk ?& array['code', 'explanation', 'evidence', 'hard_block'])
           or exists (
               select 1
               from jsonb_object_keys(
                   case when jsonb_typeof(risk) = 'object' then risk else '{}'::jsonb end
               ) as keys(key)
               where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
           )
           or jsonb_typeof(risk->'code') <> 'string'
           or coalesce(risk->>'code', '') = ''
           or jsonb_typeof(risk->'explanation') <> 'string'
           or coalesce(risk->>'explanation', '') = ''
           or jsonb_typeof(risk->'evidence') not in ('string', 'null')
           or jsonb_typeof(risk->'hard_block') <> 'boolean'
    ) then
        raise exception 'talent extraction contains an invalid risk candidate'
            using errcode = '22023';
    end if;
    if p_completion ? 'extraction_confidence'
       and (
           jsonb_typeof(p_completion->'extraction_confidence') <> 'number'
           or case
               when jsonb_typeof(p_completion->'extraction_confidence') = 'number'
                   then (p_completion->>'extraction_confidence')::numeric not between 0 and 1
               else false
           end
    ) then
        raise exception 'invalid talent extraction confidence' using errcode = '22023';
    end if;
    if p_completion ? 'contact_candidates_ciphertext'
       and jsonb_typeof(p_completion->'contact_candidates_ciphertext')
           not in ('string', 'null') then
        raise exception 'invalid encrypted talent contact payload' using errcode = '22023';
    end if;
    if nullif(p_completion->>'contact_candidates_ciphertext', '') is not null
       and (p_completion->>'contact_candidates_ciphertext') !~ '^(?:[0-9a-fA-F]{2})+$' then
        raise exception 'invalid encrypted talent contact payload' using errcode = '22023';
    end if;
    if p_completion ? 'contact_hashes'
       and jsonb_typeof(p_completion->'contact_hashes') <> 'array' then
        raise exception 'talent completion contact hashes must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'contact_hashes') = 'array'
                    then p_completion->'contact_hashes'
                else '[]'::jsonb
            end
        ) as hashes(hash)
        where jsonb_typeof(hash) <> 'string'
           or coalesce(hash #>> '{}', '') = ''
    ) then
        raise exception 'talent completion contact hashes contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_completion ? 'contact_masked'
       and jsonb_typeof(p_completion->'contact_masked') <> 'array' then
        raise exception 'talent completion masked contacts must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'contact_masked') = 'array'
                    then p_completion->'contact_masked'
                else '[]'::jsonb
            end
        ) as contacts(contact)
        where jsonb_typeof(contact) <> 'object'
           or not (contact ?& array['contact_type', 'masked_value'])
           or exists (
               select 1
               from jsonb_object_keys(
                   case when jsonb_typeof(contact) = 'object' then contact else '{}'::jsonb end
               ) as keys(key)
               where key <> all(array['contact_type', 'masked_value'])
           )
           or coalesce(contact->>'contact_type', '') not in ('phone', 'wechat', 'qq', 'other')
           or char_length(coalesce(contact->>'masked_value', '')) > 64
           or coalesce(contact->>'masked_value', '')
               !~ '^[^*[:space:][:cntrl:]]{0,4}\*{3,}[^*[:space:][:cntrl:]]{0,4}$'
    ) then
        raise exception 'invalid masked talent completion contact' using errcode = '22023';
    end if;
    if p_completion ? 'normalized_fingerprint'
       and jsonb_typeof(p_completion->'normalized_fingerprint')
           not in ('string', 'null') then
        raise exception 'invalid talent completion fingerprint' using errcode = '22023';
    end if;
    if p_completion ? 'validation_errors'
       and jsonb_typeof(p_completion->'validation_errors') <> 'array' then
        raise exception 'talent completion validation errors must be a JSON array'
            using errcode = '22023';
    end if;
    if p_completion ? 'risk_score'
       and (
           jsonb_typeof(p_completion->'risk_score') <> 'number'
           or case
               when jsonb_typeof(p_completion->'risk_score') = 'number'
                   then (p_completion->>'risk_score')::numeric not between 0 and 100
               else false
           end
       ) then
        raise exception 'invalid talent completion risk score' using errcode = '22023';
    end if;
    if p_completion ? 'risk_level'
       and (
           jsonb_typeof(p_completion->'risk_level') <> 'string'
           or p_completion->>'risk_level' not in ('low', 'medium', 'high')
       ) then
        raise exception 'invalid talent completion risk level' using errcode = '22023';
    end if;
    if p_completion ? 'risk_reasons'
       and jsonb_typeof(p_completion->'risk_reasons') <> 'array' then
        raise exception 'talent completion risk reasons must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'risk_reasons') = 'array'
                    then p_completion->'risk_reasons'
                else '[]'::jsonb
            end
        ) as reasons(reason)
        where jsonb_typeof(reason) <> 'object'
           or not (reason ?& array['code', 'explanation', 'evidence', 'hard_block'])
           or exists (
               select 1
               from jsonb_object_keys(
                   case when jsonb_typeof(reason) = 'object' then reason else '{}'::jsonb end
               ) as keys(key)
               where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
           )
           or jsonb_typeof(reason->'code') <> 'string'
           or coalesce(reason->>'code', '') = ''
           or jsonb_typeof(reason->'explanation') <> 'string'
           or coalesce(reason->>'explanation', '') = ''
           or jsonb_typeof(reason->'evidence') not in ('string', 'null')
           or jsonb_typeof(reason->'hard_block') <> 'boolean'
    ) then
        raise exception 'talent completion risk reasons contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_completion ? 'duplicate_candidates'
       and jsonb_typeof(p_completion->'duplicate_candidates') <> 'array' then
        raise exception 'talent completion duplicate candidates must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'duplicate_candidates') = 'array'
                    then p_completion->'duplicate_candidates'
                else '[]'::jsonb
            end
        ) as candidates(candidate)
        where jsonb_typeof(candidate) <> 'object'
           or not (
               candidate ?& array[
                   'candidate_item_id',
                   'candidate_record_id',
                   'score',
                   'reasons',
                   'high_similarity'
               ]
           )
           or exists (
               select 1
               from jsonb_object_keys(
                   case
                       when jsonb_typeof(candidate) = 'object' then candidate
                       else '{}'::jsonb
                   end
               ) as keys(key)
               where key <> all(
                   array[
                       'candidate_item_id',
                       'candidate_record_id',
                       'score',
                       'reasons',
                       'high_similarity'
                   ]
               )
           )
           or jsonb_typeof(candidate->'candidate_item_id') not in ('string', 'null')
           or jsonb_typeof(candidate->'candidate_record_id') not in ('string', 'null')
           or (
               jsonb_typeof(candidate->'candidate_item_id') = 'null'
               and jsonb_typeof(candidate->'candidate_record_id') = 'null'
           )
           or (
               jsonb_typeof(candidate->'candidate_item_id') = 'string'
               and (candidate->>'candidate_item_id')
                   !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           )
           or (
               jsonb_typeof(candidate->'candidate_record_id') = 'string'
               and (candidate->>'candidate_record_id')
                   !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           )
           or jsonb_typeof(candidate->'score') <> 'number'
           or case
               when jsonb_typeof(candidate->'score') = 'number'
                   then (candidate->>'score')::numeric not between 0 and 1
               else false
           end
           or jsonb_typeof(candidate->'reasons') <> 'array'
           or exists (
               select 1
               from jsonb_array_elements(
                   case
                       when jsonb_typeof(candidate->'reasons') = 'array'
                           then candidate->'reasons'
                       else '[]'::jsonb
                   end
               ) as duplicate_reasons(reason)
               where jsonb_typeof(reason) <> 'string'
                  or coalesce(reason #>> '{}', '') = ''
           )
           or jsonb_typeof(candidate->'high_similarity') <> 'boolean'
    ) then
        raise exception 'talent completion duplicate candidates contain an invalid entry'
            using errcode = '22023';
    end if;
    if not (
        p_completion ?& array[
            'item_id',
            'extraction',
            'extraction_confidence',
            'contact_candidates_ciphertext',
            'contact_hashes',
            'contact_masked',
            'normalized_fingerprint',
            'validation_errors',
            'risk_score',
            'risk_level',
            'risk_reasons',
            'duplicate_candidates'
        ]
    ) then
        raise exception 'talent import completion is missing required fields'
            using errcode = '22023';
    end if;

    update talent_import_items
    set
        status = 'needs_review',
        extraction = p_completion->'extraction',
        extraction_confidence = nullif(p_completion->>'extraction_confidence', '')::numeric,
        contact_candidates_ciphertext = nullif(p_completion->>'contact_candidates_ciphertext', ''),
        contact_hashes = array(select jsonb_array_elements_text(p_completion->'contact_hashes')),
        contact_masked = p_completion->'contact_masked',
        normalized_fingerprint = nullif(p_completion->>'normalized_fingerprint', ''),
        validation_errors = p_completion->'validation_errors',
        risk_score = nullif(p_completion->>'risk_score', '')::numeric,
        risk_level = nullif(p_completion->>'risk_level', ''),
        risk_reasons = p_completion->'risk_reasons',
        duplicate_candidates = p_completion->'duplicate_candidates',
        processed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = (p_completion->>'item_id')::uuid
      and status = 'extracting'
    returning batch_id into v_batch_id;

    if v_batch_id is null then
        raise exception 'talent import item is not extracting';
    end if;

    update talent_import_batches
    set
        processed_count = processed_count + 1,
        status = case
            when processed_count + 1 = item_count and failed_count > 0 then 'partially_failed'
            when processed_count + 1 = item_count then 'completed'
            else 'processing'
        end,
        completed_at = case
            when processed_count + 1 = item_count then current_timestamp
            else null
        end
    where batch_id = v_batch_id
      and processed_count < item_count;

    if not found then
        raise exception 'talent import batch counters are invalid';
    end if;
end;
$$ language plpgsql;

create or replace function complete_talent_source_run(p_run jsonb)
returns boolean as $$
declare
    v_success boolean := nullif(p_run->>'error_code', '') is null;
    v_subscription_id uuid;
begin
    if p_run is null or jsonb_typeof(p_run) <> 'object' then
        raise exception 'source run must be an object' using errcode = '22023';
    end if;
    update talent_source_runs set
        discovered_count = greatest(0, coalesce((p_run->>'discovered_count')::integer, 0)),
        accepted_count = greatest(0, coalesce((p_run->>'accepted_count')::integer, 0)),
        duplicate_count = greatest(0, coalesce((p_run->>'duplicate_count')::integer, 0)),
        failed_count = greatest(0, coalesce((p_run->>'failed_count')::integer, 0)),
        error_code = nullif(p_run->>'error_code', ''),
        completed_at = current_timestamp
    where run_id = (p_run->>'run_id')::uuid and completed_at is null
    returning subscription_id into v_subscription_id;
    if v_subscription_id is null then return false; end if;

    update talent_source_subscriptions set
        claimed_at = null,
        incremental_cursor = case when v_success then coalesce(nullif(p_run->>'incremental_cursor', ''), incremental_cursor) else incremental_cursor end,
        etag = case when v_success then coalesce(nullif(p_run->>'etag', ''), etag) else etag end,
        last_modified = case when v_success then coalesce(nullif(p_run->>'last_modified', ''), last_modified) else last_modified end,
        consecutive_failures = case when v_success then 0 else consecutive_failures + 1 end,
        last_success_at = case when v_success then current_timestamp else last_success_at end,
        last_error_at = case when v_success then last_error_at else current_timestamp end,
        last_error_code = nullif(p_run->>'error_code', ''),
        next_run_at = current_timestamp + case
            when v_success then make_interval(mins => interval_minutes)
            else make_interval(mins => least(interval_minutes, 15 * (1 << least(consecutive_failures, 5))))
        end,
        updated_at = current_timestamp
    where subscription_id = v_subscription_id;
    return true;
end;
$$ language plpgsql;

-- Atomically persists an encrypted import batch and all encrypted fragments.
create or replace function create_talent_import_batch(p_import jsonb)
returns uuid as $$
declare
    v_batch_id uuid;
    v_items jsonb;
begin
    if p_import is null or jsonb_typeof(p_import) <> 'object' then
        raise exception 'talent import must be a JSON object' using errcode = '22023';
    end if;

    v_items := p_import->'items';
    if v_items is null
       or jsonb_typeof(v_items) <> 'array'
       or jsonb_array_length(v_items) not between 1 and 500 then
        raise exception 'talent import must contain between 1 and 500 items' using errcode = '22023';
    end if;
    if coalesce(p_import->>'raw_text_ciphertext', '') !~ '^(?:[0-9a-fA-F]{2})+$' then
        raise exception 'invalid encrypted talent import payload' using errcode = '22023';
    end if;

    insert into talent_import_batches (
        source_id,
        imported_by,
        raw_text_ciphertext,
        encryption_key_version,
        model_provider,
        model_name,
        prompt_version,
        item_count,
        raw_delete_after
    )
    values (
        (p_import->>'source_id')::uuid,
        nullif(p_import->>'imported_by', '')::uuid,
        lower(p_import->>'raw_text_ciphertext'),
        (p_import->>'encryption_key_version')::bigint,
        nullif(p_import->>'model_provider', ''),
        nullif(p_import->>'model_name', ''),
        nullif(p_import->>'prompt_version', ''),
        jsonb_array_length(v_items),
        nullif(p_import->>'raw_delete_after', '')::timestamptz
    )
    returning batch_id into v_batch_id;

    insert into talent_import_items (
        batch_id,
        position,
        raw_fragment_ciphertext,
        encryption_key_version,
        kind
    )
    select
        v_batch_id,
        (item->>'position')::integer,
        lower(item->>'raw_fragment_ciphertext'),
        (item->>'encryption_key_version')::bigint,
        item->>'kind'
    from jsonb_array_elements(v_items) as fragments(item)
    where coalesce(item->>'raw_fragment_ciphertext', '') ~ '^(?:[0-9a-fA-F]{2})+$';

    if (select count(*) from talent_import_items where batch_id = v_batch_id) <> jsonb_array_length(v_items) then
        raise exception 'invalid encrypted talent fragment payload' using errcode = '22023';
    end if;

    return v_batch_id;
end;
$$ language plpgsql;

-- Marks extraction as failed and advances batch counters atomically.
create or replace function fail_talent_import_item(p_failure jsonb)
returns void as $$
declare
    v_batch_id uuid;
begin
    update talent_import_items
    set
        status = 'failed',
        validation_errors = coalesce(p_failure->'validation_errors', '[]'::jsonb),
        processed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = (p_failure->>'item_id')::uuid
      and status = 'extracting'
    returning batch_id into v_batch_id;

    if v_batch_id is null then
        raise exception 'talent import item is not extracting';
    end if;

    update talent_import_batches
    set
        processed_count = processed_count + 1,
        failed_count = failed_count + 1,
        status = case
            when processed_count + 1 = item_count and failed_count + 1 = item_count then 'failed'
            when processed_count + 1 = item_count then 'partially_failed'
            else 'processing'
        end,
        completed_at = case
            when processed_count + 1 = item_count then current_timestamp
            else null
        end
    where batch_id = v_batch_id
      and processed_count < item_count;

    if not found then
        raise exception 'talent import batch counters are invalid';
    end if;
end;
$$ language plpgsql;

-- Returns exact and structured candidates without raw text, ciphertext, hashes, or contacts.
drop function if exists find_talent_duplicate_candidates(uuid, integer);

create or replace function find_talent_duplicate_candidates(
    p_probe jsonb,
    p_limit integer default 20
)
returns jsonb as $$
begin
    if p_probe is null or jsonb_typeof(p_probe) <> 'object' then
        raise exception 'talent duplicate probe must be a JSON object' using errcode = '22023';
    end if;
    if not (
        p_probe ?& array[
            'item_id',
            'kind',
            'title',
            'roles',
            'city',
            'district',
            'salary_min',
            'salary_max',
            'source_id',
            'contact_hashes',
            'normalized_fingerprint',
            'normalized_text'
        ]
    ) or exists (
        select 1
        from jsonb_object_keys(p_probe) as fields(field)
        where field <> all(
            array[
                'item_id',
                'kind',
                'title',
                'roles',
                'city',
                'district',
                'salary_min',
                'salary_max',
                'source_id',
                'contact_hashes',
                'normalized_fingerprint',
                'normalized_text'
            ]
        )
    ) then
        raise exception 'talent duplicate probe has invalid fields' using errcode = '22023';
    end if;
    if jsonb_typeof(p_probe->'item_id') <> 'string'
       or (p_probe->>'item_id')
            !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
       or jsonb_typeof(p_probe->'source_id') <> 'string'
       or (p_probe->>'source_id')
            !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
       or jsonb_typeof(p_probe->'kind') <> 'string'
       or (p_probe->>'kind') not in ('hiring', 'seeking', 'irrelevant', 'uncertain')
       or jsonb_typeof(p_probe->'title') not in ('string', 'null')
       or jsonb_typeof(p_probe->'city') not in ('string', 'null')
       or jsonb_typeof(p_probe->'district') not in ('string', 'null')
       or jsonb_typeof(p_probe->'salary_min') not in ('number', 'null')
       or jsonb_typeof(p_probe->'salary_max') not in ('number', 'null')
       or jsonb_typeof(p_probe->'roles') <> 'array'
       or jsonb_typeof(p_probe->'contact_hashes') <> 'array'
       or jsonb_typeof(p_probe->'normalized_fingerprint') not in ('string', 'null')
       or jsonb_typeof(p_probe->'normalized_text') <> 'string'
    then
        raise exception 'talent duplicate probe has invalid field types' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(p_probe->'roles') as roles(role)
        where jsonb_typeof(role) <> 'string'
           or coalesce(role #>> '{}', '') = ''
    ) or exists (
        select 1
        from jsonb_array_elements(p_probe->'contact_hashes') as hashes(hash)
        where jsonb_typeof(hash) <> 'string'
           or coalesce(hash #>> '{}', '') = ''
    ) or (
        jsonb_typeof(p_probe->'normalized_fingerprint') = 'string'
        and coalesce(p_probe->>'normalized_fingerprint', '') = ''
    ) then
        raise exception 'talent duplicate probe has invalid array entries' using errcode = '22023';
    end if;
    if (
        jsonb_typeof(p_probe->'salary_min') = 'number'
        and (
            (p_probe->>'salary_min')::numeric % 1 <> 0
            or (p_probe->>'salary_min')::numeric not between 0 and 2147483647
        )
    ) or (
        jsonb_typeof(p_probe->'salary_max') = 'number'
        and (
            (p_probe->>'salary_max')::numeric % 1 <> 0
            or (p_probe->>'salary_max')::numeric not between 0 and 2147483647
        )
    ) then
        raise exception 'talent duplicate probe has invalid salary values' using errcode = '22023';
    end if;

    return (
with subject as (
    select
        (p_probe->>'item_id')::uuid as item_id,
        nullif(p_probe->>'normalized_fingerprint', '') as normalized_fingerprint,
        array(
            select hash #>> '{}'
            from jsonb_array_elements(p_probe->'contact_hashes') as hashes(hash)
        ) as contact_hashes,
        p_probe->>'kind' as kind,
        (p_probe->>'source_id')::uuid as source_id,
        p_probe->'roles' as roles,
        nullif(p_probe->>'city', '') as city,
        nullif(p_probe->>'district', '') as district,
        nullif(p_probe->>'salary_min', '')::integer as salary_min,
        nullif(p_probe->>'salary_max', '')::integer as salary_max,
        p_probe->>'normalized_text' as normalized_text
),
item_candidates as (
    select
        candidate.item_id as candidate_item_id,
        candidate.published_record_id as candidate_record_id,
        candidate.kind,
        candidate.status,
        case
            when jsonb_typeof(candidate.extraction->'title') = 'string'
                then candidate.extraction->>'title'
        end as title,
        coalesce(
            (
                select jsonb_agg(role #>> '{}')
                from jsonb_array_elements(
                    case
                        when jsonb_typeof(candidate.extraction->'roles') = 'array'
                            then candidate.extraction->'roles'
                        else '[]'::jsonb
                    end
                ) role
                where jsonb_typeof(role) = 'string'
            ),
            '[]'::jsonb
        ) as roles,
        case
            when jsonb_typeof(candidate.extraction->'city') = 'string'
                then candidate.extraction->>'city'
        end as city,
        case
            when jsonb_typeof(candidate.extraction->'district') = 'string'
                then candidate.extraction->>'district'
        end as district,
        case
            when jsonb_typeof(coalesce(
                candidate.extraction->'salaryMin',
                candidate.extraction->'salary_min'
            )) = 'number'
                then coalesce(
                    candidate.extraction->>'salaryMin',
                    candidate.extraction->>'salary_min'
                )::integer
        end as salary_min,
        case
            when jsonb_typeof(coalesce(
                candidate.extraction->'salaryMax',
                candidate.extraction->'salary_max'
            )) = 'number'
                then coalesce(
                    candidate.extraction->>'salaryMax',
                    candidate.extraction->>'salary_max'
                )::integer
        end as salary_max,
        candidate_batch.source_id,
        lower(
            regexp_replace(
                concat_ws(
                    ' ',
                    case
                        when jsonb_typeof(candidate.extraction->'title') = 'string'
                            then candidate.extraction->>'title'
                    end,
                    (
                        select string_agg(role #>> '{}', ' ')
                        from jsonb_array_elements(
                            case
                                when jsonb_typeof(candidate.extraction->'roles') = 'array'
                                    then candidate.extraction->'roles'
                                else '[]'::jsonb
                            end
                        ) role
                        where jsonb_typeof(role) = 'string'
                    ),
                    case
                        when jsonb_typeof(candidate.extraction->'city') = 'string'
                            then candidate.extraction->>'city'
                    end,
                    case
                        when jsonb_typeof(candidate.extraction->'district') = 'string'
                            then candidate.extraction->>'district'
                    end
                ),
                '\s+',
                ' ',
                'g'
            )
        ) as normalized_text,
        case
            when subject.normalized_fingerprint is not null
             and candidate.normalized_fingerprint is not null
             and candidate.normalized_fingerprint = subject.normalized_fingerprint then 1.0
            when coalesce(candidate.contact_hashes && subject.contact_hashes, false) then 1.0
            else 0.0
        end as score,
        case
            when subject.normalized_fingerprint is not null
             and candidate.normalized_fingerprint is not null
             and candidate.normalized_fingerprint = subject.normalized_fingerprint
                then jsonb_build_array('exact_fingerprint')
            when coalesce(candidate.contact_hashes && subject.contact_hashes, false)
                then jsonb_build_array('exact_contact_hash')
            else '[]'::jsonb
        end as reasons,
        (
            (
                subject.normalized_fingerprint is not null
                and candidate.normalized_fingerprint is not null
                and candidate.normalized_fingerprint = subject.normalized_fingerprint
            )
            or coalesce(candidate.contact_hashes && subject.contact_hashes, false)
        ) as high_similarity,
        candidate.updated_at
    from subject
    join talent_import_items candidate
      on candidate.item_id <> subject.item_id
    join talent_import_batches candidate_batch
      on candidate_batch.batch_id = candidate.batch_id
    where (
        (
            subject.normalized_fingerprint is not null
            and candidate.normalized_fingerprint = subject.normalized_fingerprint
        )
        or (
            coalesce(cardinality(subject.contact_hashes), 0) > 0
            and candidate.contact_hashes && subject.contact_hashes
        )
        or (
            candidate.extraction is not null
            and candidate.kind = subject.kind
        )
      )
),
record_candidates as (
    select
        null::uuid as candidate_item_id,
        record.record_id as candidate_record_id,
        record.kind,
        record.status,
        record.title,
        to_jsonb(coalesce(record.roles, '{}'::text[])) as roles,
        record.city,
        record.district,
        record.salary_min,
        record.salary_max,
        record.source_id,
        lower(
            regexp_replace(
                concat_ws(
                    ' ',
                    record.title,
                    array_to_string(record.roles, ' '),
                    record.city,
                    record.district
                ),
                '\s+',
                ' ',
                'g'
            )
        ) as normalized_text,
        case
            when exact.fingerprint_match or exact.contact_match then 1.0
            else 0.0
        end as score,
        case
            when exact.fingerprint_match then jsonb_build_array('exact_fingerprint')
            when exact.contact_match then jsonb_build_array('exact_contact_hash')
            else '[]'::jsonb
        end as reasons,
        (exact.fingerprint_match or exact.contact_match) as high_similarity,
        record.updated_at
    from subject
    join talent_records record
      on record.status <> 'withdrawn'
    cross join lateral (
        select
            exists (
                select 1
                from talent_import_items origin
                where origin.published_record_id = record.record_id
                  and subject.normalized_fingerprint is not null
                  and origin.normalized_fingerprint = subject.normalized_fingerprint
            ) as fingerprint_match,
            exists (
                select 1
                from talent_contacts contact
                where contact.record_id = record.record_id
                  and contact.deleted_at is null
                  and coalesce(cardinality(subject.contact_hashes), 0) > 0
                  and contact.normalized_hash = any(subject.contact_hashes)
            ) as contact_match
    ) exact
    where record.record_id is distinct from (
        select own.published_record_id
        from talent_import_items own
        where own.item_id = subject.item_id
    )
      and (record.kind = subject.kind or exact.fingerprint_match or exact.contact_match)
      and not exists (
        select 1
        from talent_import_items represented
        where represented.published_record_id = record.record_id
      )
),
candidates as (
    select * from item_candidates
    union all
    select * from record_candidates
),
-- Preselection uses a conservative text upper bound; Rust recomputes the authoritative score.
ranked_candidates as (
    select
        candidates.*,
        (
            case
                when exists (
                    select 1
                    from jsonb_array_elements_text(subject.roles) as subject_role(value)
                    join jsonb_array_elements_text(candidates.roles) as candidate_role(value)
                      on lower(btrim(subject_role.value)) = lower(btrim(candidate_role.value))
                     and btrim(subject_role.value) <> ''
                ) then 0.35
                else 0.0
            end
            + case
                when (
                    lower(btrim(subject.city)) = lower(btrim(candidates.city))
                    and btrim(subject.city) <> ''
                ) or (
                    lower(btrim(subject.district)) = lower(btrim(candidates.district))
                    and btrim(subject.district) <> ''
                ) then 0.25
                else 0.0
            end
            + case
                when subject.salary_min is not null
                 and subject.salary_max is not null
                 and candidates.salary_min is not null
                 and candidates.salary_max is not null
                 and subject.salary_min <= candidates.salary_max
                 and candidates.salary_min <= subject.salary_max then 0.20
                else 0.0
            end
            + case
                when candidates.source_id = subject.source_id then 0.10
                else 0.0
            end
            + case
                when coalesce(subject.normalized_text, '') <> ''
                 and coalesce(candidates.normalized_text, '') <> '' then 0.10
                else 0.0
            end
        ) as structured_upper_bound
    from candidates
    cross join subject
)
select coalesce(
    jsonb_agg(
        jsonb_build_object(
            'candidate_item_id', candidate_item_id,
            'candidate_record_id', candidate_record_id,
            'kind', kind,
            'status', status,
            'title', title,
            'roles', roles,
            'city', city,
            'district', district,
            'salary_min', salary_min,
            'salary_max', salary_max,
            'source_id', source_id,
            'normalized_text', normalized_text,
            'score', score,
            'reasons', reasons,
            'high_similarity', high_similarity
        )
        order by score desc, structured_upper_bound desc, updated_at desc
    ),
    '[]'::jsonb
)
from (
    select *
    from ranked_candidates
    order by score desc, structured_upper_bound desc, updated_at desc
    limit greatest(0, least(coalesce(p_limit, 20), 100))
) limited
    );
end;
$$ language plpgsql stable;

-- Returns one private review detail. Ciphertext remains encrypted and is never logged or public.
create or replace function get_talent_review_item(p_item_id uuid)
returns jsonb as $$
    select to_jsonb(detail)
    from (
        select
            i.item_id,
            i.batch_id,
            i.position,
            i.raw_fragment_ciphertext,
            i.encryption_key_version,
            i.contact_candidates_ciphertext,
            coalesce(i.contact_hashes, '{}'::text[]) as contact_hashes,
            i.contact_masked,
            i.normalized_fingerprint,
            i.kind,
            i.status,
            i.extraction,
            i.extraction_confidence,
            i.validation_errors,
            i.risk_score,
            i.risk_level,
            i.risk_reasons,
            i.duplicate_candidates,
            i.published_record_id,
            i.lock_version,
            i.processed_at,
            i.reviewed_at,
            i.updated_at,
            b.model_provider,
            b.model_name,
            b.prompt_version,
            s.source_id,
            s.source_type,
            s.name as source_name,
            s.trust_status as source_trust_status,
            s.auto_publish_enabled as source_auto_publish_enabled,
            s.confirmed_at as source_confirmed_at
        from talent_import_items i
        join talent_import_batches b using (batch_id)
        join talent_sources s using (source_id)
        where i.item_id = p_item_id
    ) detail;
$$ language sql stable;

-- Returns non-secret runtime publication controls.
create or replace function get_talent_runtime_settings()
returns jsonb as $$
    select jsonb_build_object(
        'auto_publish_enabled', auto_publish_enabled,
        'updated_by', updated_by,
        'updated_at', updated_at
    )
    from talent_runtime_settings
    where singleton;
$$ language sql stable;

-- Public projection: explicitly excludes raw text, ciphertext, hashes, and full contacts.
create or replace function list_published_talent_records(
    p_kind text default null,
    p_limit integer default 50
)
returns jsonb as $$
    select coalesce(
        jsonb_agg(
            jsonb_build_object(
                'record_id', records.record_id,
                'kind', records.kind,
                'title', records.title,
                'scene', records.scene,
                'region', records.region,
                'city', records.city,
                'district', records.district,
                'salary_min', records.salary_min,
                'salary_max', records.salary_max,
                'roles', records.roles,
                'skills', records.skills,
                'experience_years', records.experience_years,
                'hours_per_day', records.hours_per_day,
                'rest_days_per_month', records.rest_days_per_month,
                'lodging', records.lodging,
                'housing_allowance', records.housing_allowance,
                'availability', records.availability,
                'urgency', records.urgency,
                'source_label', records.source_label,
                'published_at', records.published_at,
                'expires_at', records.expires_at,
                'contact_masked', records.contact_masked
            )
            order by records.published_at desc, records.record_id
        ),
        '[]'::jsonb
    )
    from (
        select
            r.record_id,
            r.kind,
            r.title,
            r.scene,
            r.region,
            r.city,
            r.district,
            r.salary_min,
            r.salary_max,
            coalesce(r.roles, '{}'::text[]) as roles,
            coalesce(r.skills, '{}'::text[]) as skills,
            r.experience_years,
            r.hours_per_day,
            r.rest_days_per_month,
            r.lodging,
            r.housing_allowance,
            r.availability,
            r.urgency,
            r.source_label,
            coalesce(r.published_at, r.created_at) as published_at,
            r.expires_at,
            coalesce(
                (
                    select jsonb_agg(
                        jsonb_build_object(
                            'contact_type', c.contact_type,
                            'masked_value', c.masked_value
                        )
                        order by c.created_at, c.contact_id
                    )
                    from talent_contacts c
                    where c.record_id = r.record_id
                      and c.deleted_at is null
                      and char_length(c.masked_value) <= 64
                      and c.masked_value
                          ~ '^[^*[:space:][:cntrl:]]{0,4}\*{3,}[^*[:space:][:cntrl:]]{0,4}$'
                ),
                '[]'::jsonb
            ) as contact_masked
        from talent_records r
        where r.status = 'published'
          and (p_kind is null or r.kind = p_kind)
          and (r.expires_at is null or r.expires_at > current_timestamp)
        order by r.published_at desc, r.record_id
        limit greatest(0, least(coalesce(p_limit, 50), 500))
    ) records;
$$ language sql stable;

-- Lists recent imports with safe source metadata and processing counters.
create or replace function list_talent_import_batches(p_limit integer)
returns jsonb as $$
    select coalesce(
        jsonb_agg(to_jsonb(summary) order by summary.created_at desc, summary.batch_id desc),
        '[]'::jsonb
    )
    from (
        select
            b.batch_id,
            b.source_id,
            s.source_type,
            s.name as source_name,
            s.trust_status as source_trust_status,
            b.status,
            b.item_count,
            b.processed_count,
            b.failed_count,
            b.model_provider,
            b.model_name,
            b.prompt_version,
            b.created_at,
            b.completed_at
        from talent_import_batches b
        join talent_sources s using (source_id)
        order by b.created_at desc, b.batch_id desc
        limit greatest(0, least(coalesce(p_limit, 50), 500))
    ) summary;
$$ language sql stable;

-- Lists review-safe import summaries without raw or complete-contact material.
create or replace function list_talent_review_items(
    p_statuses text[] default null,
    p_source_id uuid default null,
    p_limit integer default 50
)
returns jsonb as $$
    select coalesce(
        jsonb_agg(to_jsonb(summary) order by summary.updated_at desc, summary.item_id),
        '[]'::jsonb
    )
    from (
        select
            i.item_id,
            i.batch_id,
            i.position,
            i.kind,
            i.status,
            i.extraction,
            i.extraction_confidence,
            i.contact_masked,
            coalesce(cardinality(i.contact_hashes), 0) > 0 as has_contact,
            i.validation_errors,
            i.risk_score,
            i.risk_level,
            i.risk_reasons,
            i.duplicate_candidates,
            i.lock_version,
            i.processed_at,
            i.reviewed_at,
            i.updated_at,
            s.source_id,
            s.source_type,
            s.name as source_name,
            s.trust_status as source_trust_status
        from talent_import_items i
        join talent_import_batches b using (batch_id)
        join talent_sources s using (source_id)
        where (
            p_statuses is null
            or cardinality(p_statuses) = 0
            or i.status = any(p_statuses)
        )
          and (p_source_id is null or s.source_id = p_source_id)
        order by i.updated_at desc, i.item_id
        limit greatest(0, least(coalesce(p_limit, 50), 500))
    ) summary;
$$ language sql stable;

create or replace function list_talent_source_subscriptions()
returns jsonb as $$
    select coalesce(jsonb_agg(row_data order by row_data->>'created_at'), '[]'::jsonb)
    from (
        select jsonb_build_object(
            'subscription_id', subscription.subscription_id,
            'source_id', subscription.source_id,
            'source_name', source.name,
            'public_url', subscription.public_url,
            'keywords', subscription.keywords,
            'interval_minutes', subscription.interval_minutes,
            'enabled', subscription.enabled,
            'next_run_at', subscription.next_run_at,
            'incremental_cursor', subscription.incremental_cursor,
            'etag', subscription.etag,
            'last_modified', subscription.last_modified,
            'consecutive_failures', subscription.consecutive_failures,
            'last_success_at', subscription.last_success_at,
            'last_error_at', subscription.last_error_at,
            'last_error_code', subscription.last_error_code,
            'created_by', subscription.created_by,
            'created_at', subscription.created_at
        ) row_data
        from talent_source_subscriptions subscription
        join talent_sources source using (source_id)
    ) rows;
$$ language sql stable;

-- Marks a review item as a duplicate of an existing formal record atomically.
create or replace function merge_talent_import_item(
    p_item_id uuid,
    p_lock_version integer,
    p_target_record_id uuid,
    p_actor_user_id uuid,
    p_reason text
)
returns boolean as $$
declare
    v_item talent_import_items%rowtype;
begin
    if p_reason is not null
       and p_reason not in (
           'duplicate',
           'rejected',
           'reprocess',
           'withdrawn',
           'moderator_correction',
           'policy_violation',
           'expired',
           'other'
       ) then
        raise exception 'invalid talent audit reason' using errcode = '22023';
    end if;

    select *
    into v_item
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if v_item.lock_version <> p_lock_version then
        return false;
    end if;
    if v_item.status <> 'needs_review' then
        raise exception 'talent import item is not awaiting review';
    end if;
    if not exists (
        select 1
        from talent_records
        where record_id = p_target_record_id
          and status <> 'withdrawn'
    ) then
        raise exception 'talent merge target not found';
    end if;

    update talent_import_items
    set
        status = 'merged',
        duplicate_candidates = coalesce(duplicate_candidates, '[]'::jsonb)
            || jsonb_build_array(
                jsonb_build_object(
                    'candidate_item_id', null,
                    'candidate_record_id', p_target_record_id,
                    'score', 1,
                    'reasons', jsonb_build_array('moderator_merge'),
                    'high_similarity', true
                )
            ),
        lock_version = lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = p_item_id;

    insert into talent_review_events (
        item_id,
        record_id,
        actor_type,
        actor_user_id,
        action,
        reason,
        before_snapshot,
        after_snapshot
    )
    values (
        p_item_id,
        p_target_record_id,
        'moderator',
        p_actor_user_id,
        'merged',
        p_reason,
        jsonb_build_object('status', v_item.status, 'lock_version', v_item.lock_version),
        jsonb_build_object(
            'status', 'merged',
            'target_record_id', p_target_record_id,
            'lock_version', v_item.lock_version + 1
        )
    );

    return true;
end;
$$ language plpgsql;

-- Publishes an item and its encrypted contacts atomically and idempotently.
create or replace function publish_talent_item(
    p_item_id uuid,
    p_lock_version integer,
    p_actor_user_id uuid,
    p_automatic boolean,
    p_record jsonb,
    p_contacts jsonb
)
returns table(record_id uuid, created boolean) as $$
declare
    v_item talent_import_items%rowtype;
    v_batch talent_import_batches%rowtype;
    v_source talent_sources%rowtype;
    v_record_id uuid;
    v_contact_count integer;
    v_global_enabled boolean;
    v_publication jsonb;
    v_required_complete boolean;
    v_safety_valid boolean;
    v_hard_block boolean;
    v_high_duplicate boolean;
    v_fresh_duplicate boolean;
begin
    select *
    into v_item
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if p_lock_version is null then
        raise exception 'talent import item lock version is required' using errcode = '22023';
    end if;
    if p_automatic is null then
        raise exception 'talent publication mode is required' using errcode = '22023';
    end if;

    -- A retry may carry the original lock version. The durable source link wins.
    if v_item.published_record_id is not null
       and v_item.status in ('published', 'auto_published') then
        record_id := v_item.published_record_id;
        created := false;
        return next;
        return;
    end if;

    if v_item.lock_version <> p_lock_version then
        raise exception 'talent import item lock conflict' using errcode = '40001';
    end if;
    if v_item.status <> 'needs_review' then
        raise exception 'talent import item is not awaiting review';
    end if;
    if p_record is null or jsonb_typeof(p_record) <> 'object' then
        raise exception 'talent publication record must be a JSON object' using errcode = '22023';
    end if;
    if p_contacts is null or jsonb_typeof(p_contacts) <> 'array' then
        raise exception 'talent publication contacts must be a JSON array' using errcode = '22023';
    end if;
    if p_record ? 'contacts' then
        raise exception 'complete contacts are forbidden in talent publication record' using errcode = '22023';
    end if;
    if not p_automatic and p_actor_user_id is null then
        raise exception 'manual talent publication requires a Moderator' using errcode = '22023';
    end if;

    select *
    into v_batch
    from talent_import_batches
    where batch_id = v_item.batch_id;

    select *
    into v_source
    from talent_sources
    where source_id = v_batch.source_id
    for update;

    select auto_publish_enabled
    into v_global_enabled
    from talent_runtime_settings
    where singleton
    for update;

    v_publication := case
        when p_automatic then jsonb_build_object(
            'kind', v_item.kind,
            'title', v_item.extraction->>'title',
            'scene', v_item.extraction->>'scene',
            'region', v_item.extraction->>'region',
            'city', v_item.extraction->>'city',
            'district', v_item.extraction->>'district',
            'salary_min', coalesce(
                v_item.extraction->'salaryMin',
                v_item.extraction->'salary_min'
            ),
            'salary_max', coalesce(
                v_item.extraction->'salaryMax',
                v_item.extraction->'salary_max'
            ),
            'roles', coalesce(v_item.extraction->'roles', '[]'::jsonb),
            'skills', coalesce(v_item.extraction->'skills', '[]'::jsonb),
            'experience_years', coalesce(
                v_item.extraction->'experienceYears',
                v_item.extraction->'experience_years'
            ),
            'hours_per_day', coalesce(
                v_item.extraction->'hoursPerDay',
                v_item.extraction->'hours_per_day'
            ),
            'rest_days_per_month', coalesce(
                v_item.extraction->'restDaysPerMonth',
                v_item.extraction->'rest_days_per_month'
            ),
            'availability', v_item.extraction->>'availability',
            'urgency', v_item.extraction->>'urgency',
            'published_at', coalesce(
                v_item.extraction->'publishedAt',
                v_item.extraction->'published_at'
            ),
            'expires_at', coalesce(
                v_item.extraction->'expiresAt',
                v_item.extraction->'expires_at'
            )
        )
        else p_record
    end;

    v_contact_count := jsonb_array_length(p_contacts);
    v_safety_valid :=
        jsonb_typeof(v_item.risk_reasons) = 'array'
        and not exists (
            select 1
            from jsonb_array_elements(
                case
                    when jsonb_typeof(v_item.risk_reasons) = 'array' then v_item.risk_reasons
                    else '[]'::jsonb
                end
            ) as reasons(reason)
            where jsonb_typeof(reason) <> 'object'
               or not (reason ?& array['code', 'explanation', 'evidence', 'hard_block'])
               or exists (
                   select 1
                   from jsonb_object_keys(
                       case when jsonb_typeof(reason) = 'object' then reason else '{}'::jsonb end
                   ) as keys(key)
                   where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
               )
               or jsonb_typeof(reason->'code') <> 'string'
               or coalesce(reason->>'code', '') = ''
               or jsonb_typeof(reason->'explanation') <> 'string'
               or coalesce(reason->>'explanation', '') = ''
               or jsonb_typeof(reason->'evidence') not in ('string', 'null')
               or jsonb_typeof(reason->'hard_block') <> 'boolean'
        )
        and jsonb_typeof(v_item.duplicate_candidates) = 'array'
        and not exists (
            select 1
            from jsonb_array_elements(
                case
                    when jsonb_typeof(v_item.duplicate_candidates) = 'array'
                        then v_item.duplicate_candidates
                    else '[]'::jsonb
                end
            ) as candidates(candidate)
            where jsonb_typeof(candidate) <> 'object'
               or not (
                   candidate ?& array[
                       'candidate_item_id',
                       'candidate_record_id',
                       'score',
                       'reasons',
                       'high_similarity'
                   ]
               )
               or exists (
                   select 1
                   from jsonb_object_keys(
                       case
                           when jsonb_typeof(candidate) = 'object' then candidate
                           else '{}'::jsonb
                       end
                   ) as keys(key)
                   where key <> all(
                       array[
                           'candidate_item_id',
                           'candidate_record_id',
                           'score',
                           'reasons',
                           'high_similarity'
                       ]
                   )
               )
               or jsonb_typeof(candidate->'candidate_item_id') not in ('string', 'null')
               or jsonb_typeof(candidate->'candidate_record_id') not in ('string', 'null')
               or (
                   jsonb_typeof(candidate->'candidate_item_id') = 'null'
                   and jsonb_typeof(candidate->'candidate_record_id') = 'null'
               )
               or (
                   jsonb_typeof(candidate->'candidate_item_id') = 'string'
                   and (candidate->>'candidate_item_id')
                       !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               )
               or (
                   jsonb_typeof(candidate->'candidate_record_id') = 'string'
                   and (candidate->>'candidate_record_id')
                       !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               )
               or jsonb_typeof(candidate->'score') <> 'number'
               or case
                   when jsonb_typeof(candidate->'score') = 'number'
                       then (candidate->>'score')::numeric not between 0 and 1
                   else false
               end
               or jsonb_typeof(candidate->'reasons') <> 'array'
               or exists (
                   select 1
                   from jsonb_array_elements(
                       case
                           when jsonb_typeof(candidate->'reasons') = 'array'
                               then candidate->'reasons'
                           else '[]'::jsonb
                       end
                   ) as duplicate_reasons(reason)
                   where jsonb_typeof(reason) <> 'string'
                      or coalesce(reason #>> '{}', '') = ''
               )
               or jsonb_typeof(candidate->'high_similarity') <> 'boolean'
        );

    v_hard_block := case
        when v_safety_valid then jsonb_path_exists(
            v_item.risk_reasons,
            '$[*] ? (@.hard_block == true)'
        )
        else true
    end;
    v_high_duplicate := case
        when v_safety_valid then jsonb_path_exists(
            v_item.duplicate_candidates,
            '$[*] ? (@.high_similarity == true || @.score >= 0.85)'
        )
        else true
    end;
    v_required_complete :=
        coalesce(v_item.extraction->>'kind', '') = v_item.kind
        and coalesce(nullif(v_publication->>'title', ''), '') <> ''
        and coalesce(nullif(v_publication->>'published_at', ''), '') <> ''
        and case
            when jsonb_typeof(v_publication->'roles') = 'array'
                then jsonb_array_length(v_publication->'roles') > 0
            else false
        end
        and not exists (
            select 1
            from jsonb_array_elements(
                case
                    when jsonb_typeof(v_publication->'roles') = 'array'
                        then v_publication->'roles'
                    else '[]'::jsonb
                end
            ) as roles(role)
            where jsonb_typeof(role) <> 'string'
               or coalesce(role #>> '{}', '') = ''
        )
        and (
            coalesce(nullif(v_publication->>'city', ''), '') <> ''
            or coalesce(nullif(v_publication->>'district', ''), '') <> ''
        )
        and v_contact_count > 0
        and coalesce(cardinality(v_item.contact_hashes), 0) > 0
        and not exists (
            select 1
            from jsonb_array_elements(p_contacts) as required_contacts(contact)
            where nullif(contact->>'normalized_hash', '') is null
               or not (
                   (contact->>'normalized_hash') = any(
                       coalesce(v_item.contact_hashes, '{}'::text[])
                   )
               )
        )
        and case
            when v_item.kind = 'hiring' then
                nullif(v_publication->>'salary_min', '') is not null
                and nullif(v_publication->>'salary_max', '') is not null
            when v_item.kind = 'seeking' then true
            else false
        end
        and (
            v_item.validation_errors is null
            or case
                when jsonb_typeof(v_item.validation_errors) = 'array'
                    then jsonb_array_length(v_item.validation_errors) = 0
                else false
            end
        )
        and (
            coalesce(
                v_item.extraction->'missingFields',
                v_item.extraction->'missing_fields'
            ) is null
            or case
                when jsonb_typeof(
                    coalesce(
                        v_item.extraction->'missingFields',
                        v_item.extraction->'missing_fields'
                    )
                ) = 'array'
                    then jsonb_array_length(
                    coalesce(
                        v_item.extraction->'missingFields',
                        v_item.extraction->'missing_fields'
                    )
                ) = 0
                else false
            end
        );

    if p_automatic then
        -- Serialize all automatic publications sharing an exact dedupe key. The
        -- lock names are one-way database hashes and are held for this transaction.
        perform pg_advisory_xact_lock(hashtextextended(dedupe_key, 0))
        from (
            select 'talent-contact:' || contact_hash as dedupe_key
            from unnest(coalesce(v_item.contact_hashes, '{}'::text[])) as hashes(contact_hash)
            where contact_hash <> ''
            union
            select 'talent-fingerprint:' || v_item.normalized_fingerprint
            where nullif(v_item.normalized_fingerprint, '') is not null
        ) exact_keys
        order by dedupe_key;

        select exists (
            select 1
            from talent_contacts contact
            join talent_records record using (record_id)
            where record.status <> 'withdrawn'
              and contact.deleted_at is null
              and contact.normalized_hash = any(coalesce(v_item.contact_hashes, '{}'::text[]))
            union all
            select 1
            from talent_import_items origin
            join talent_records record
              on record.record_id = origin.published_record_id
            where origin.item_id <> v_item.item_id
              and origin.status in ('published', 'auto_published')
              and record.status <> 'withdrawn'
              and nullif(v_item.normalized_fingerprint, '') is not null
              and origin.normalized_fingerprint = v_item.normalized_fingerprint
        )
        into v_fresh_duplicate;
    else
        v_fresh_duplicate := false;
    end if;

    if p_automatic and not coalesce((
        v_item.risk_level = 'low'
        and not v_hard_block
        and v_source.trust_status = 'trusted'
        and v_source.auto_publish_enabled
        and v_source.confirmed_by is not null
        and v_source.confirmed_at is not null
        and coalesce(v_global_enabled, false)
        and coalesce(v_item.extraction_confidence, 0) >= 0.90
        and v_safety_valid
        and v_required_complete
        and not v_high_duplicate
        and not v_fresh_duplicate
    ), false) then
        raise exception 'automatic talent publication gates failed' using errcode = '22023';
    end if;

    if exists (
        select 1
        from jsonb_array_elements(p_contacts) as contacts(contact)
        where jsonb_typeof(contact) <> 'object'
           or contact ?| array['value', 'complete_contact', 'raw']
           or coalesce(contact->>'contact_type', '') not in ('phone', 'wechat', 'qq', 'other')
           or coalesce(contact->>'ciphertext', '') !~ '^(?:[0-9a-fA-F]{2})+$'
           or coalesce(nullif(contact->>'encryption_key_version', '')::bigint, 0) < 1
           or coalesce(nullif(contact->>'encryption_key_version', '')::numeric, 0)
                > 4294967295
           or char_length(coalesce(contact->>'masked_value', '')) > 64
           or coalesce(contact->>'masked_value', '')
               !~ '^[^*[:space:][:cntrl:]]{0,4}\*{3,}[^*[:space:][:cntrl:]]{0,4}$'
    ) then
        if exists (
            select 1
            from jsonb_array_elements(p_contacts) as masked_contacts(contact)
            where char_length(coalesce(contact->>'masked_value', '')) > 64
               or coalesce(contact->>'masked_value', '')
                   !~ '^[^*[:space:][:cntrl:]]{0,4}\*{3,}[^*[:space:][:cntrl:]]{0,4}$'
        ) then
            raise exception 'invalid masked talent publication contact' using errcode = '22023';
        end if;
        raise exception 'invalid encrypted talent publication contact' using errcode = '22023';
    end if;

    insert into talent_records (
        kind,
        title,
        scene,
        region,
        city,
        district,
        salary_min,
        salary_max,
        roles,
        skills,
        experience_years,
        hours_per_day,
        rest_days_per_month,
        lodging,
        housing_allowance,
        availability,
        urgency,
        source_id,
        source_label,
        published_at,
        expires_at,
        status,
        creation_source
    )
    values (
        coalesce(v_publication->>'kind', v_item.kind),
        v_publication->>'title',
        nullif(v_publication->>'scene', ''),
        nullif(v_publication->>'region', ''),
        nullif(v_publication->>'city', ''),
        nullif(v_publication->>'district', ''),
        nullif(v_publication->>'salary_min', '')::integer,
        nullif(v_publication->>'salary_max', '')::integer,
        coalesce(
            array(select jsonb_array_elements_text(coalesce(v_publication->'roles', '[]'::jsonb))),
            '{}'::text[]
        ),
        coalesce(
            array(select jsonb_array_elements_text(coalesce(v_publication->'skills', '[]'::jsonb))),
            '{}'::text[]
        ),
        nullif(v_publication->>'experience_years', '')::integer,
        nullif(v_publication->>'hours_per_day', '')::integer,
        nullif(v_publication->>'rest_days_per_month', '')::integer,
        nullif(v_publication->>'lodging', '')::boolean,
        nullif(v_publication->>'housing_allowance', '')::boolean,
        nullif(v_publication->>'availability', ''),
        nullif(v_publication->>'urgency', ''),
        v_source.source_id,
        coalesce(nullif(v_publication->>'source_label', ''), v_source.name),
        coalesce(nullif(v_publication->>'published_at', '')::timestamptz, current_timestamp),
        nullif(v_publication->>'expires_at', '')::timestamptz,
        'published',
        case when p_automatic then 'auto_publish' else 'manual_review' end
    )
    returning talent_records.record_id into v_record_id;

    insert into talent_contacts (
        record_id,
        contact_type,
        ciphertext,
        encryption_key_version,
        normalized_hash,
        masked_value
    )
    select
        v_record_id,
        contact->>'contact_type',
        lower(contact->>'ciphertext'),
        (contact->>'encryption_key_version')::bigint,
        nullif(contact->>'normalized_hash', ''),
        contact->>'masked_value'
    from jsonb_array_elements(p_contacts) as contacts(contact);

    insert into talent_review_events (
        item_id,
        record_id,
        actor_type,
        actor_user_id,
        action,
        before_snapshot,
        after_snapshot,
        model_provider,
        model_name,
        prompt_version,
        rule_version,
        policy_version,
        automatic_reason
    )
    values (
        p_item_id,
        v_record_id,
        case when p_automatic then 'system' else 'moderator' end,
        case when p_automatic then null else p_actor_user_id end,
        case when p_automatic then 'auto_published' else 'published' end,
        jsonb_build_object(
            'status', v_item.status,
            'lock_version', v_item.lock_version,
            'risk_level', v_item.risk_level,
            'risk_score', v_item.risk_score
        ),
        jsonb_build_object(
            'status', case when p_automatic then 'auto_published' else 'published' end,
            'record_id', v_record_id,
            'kind', coalesce(v_publication->>'kind', v_item.kind),
            'title', v_publication->>'title',
            'source_id', v_source.source_id
        ),
        v_batch.model_provider,
        v_batch.model_name,
        v_batch.prompt_version,
        'talent-risk-v1',
        'talent-auto-publish-v1',
        case when p_automatic
            then 'all automatic gates passed'
            else null
        end
    );

    update talent_import_items
    set
        status = case when p_automatic then 'auto_published' else 'published' end,
        published_record_id = v_record_id,
        lock_version = lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = p_item_id;

    update talent_sources
    set published_count = published_count + 1, updated_at = current_timestamp
    where source_id = v_source.source_id;

    record_id := v_record_id;
    created := true;
    return next;
end;
$$ language plpgsql;

create or replace function release_talent_source_item(
    p_subscription_id uuid,
    p_source_item_id text,
    p_content_fingerprint text
)
returns boolean as $$
declare
    v_deleted integer;
begin
    delete from talent_source_seen_items
    where subscription_id = p_subscription_id
      and source_item_id = p_source_item_id
      and content_fingerprint = p_content_fingerprint;
    get diagnostics v_deleted = row_count;
    return v_deleted > 0;
end;
$$ language plpgsql;

create or replace function reserve_talent_source_item(
    p_subscription_id uuid,
    p_source_item_id text,
    p_content_fingerprint text
)
returns boolean as $$
declare
    v_inserted integer;
begin
    if trim(coalesce(p_source_item_id, '')) = ''
       or p_content_fingerprint !~ '^[0-9a-f]{64}$' then
        raise exception 'invalid source item identity' using errcode = '22023';
    end if;
    insert into talent_source_seen_items (subscription_id, source_item_id, content_fingerprint)
    values (p_subscription_id, p_source_item_id, p_content_fingerprint)
    on conflict do nothing;
    get diagnostics v_inserted = row_count;
    return v_inserted > 0;
end;
$$ language plpgsql;

-- Sets the singleton emergency auto-publication switch.
create or replace function set_talent_auto_publish(
    p_enabled boolean,
    p_actor_user_id uuid
)
returns jsonb as $$
    update talent_runtime_settings
    set
        auto_publish_enabled = p_enabled,
        updated_by = p_actor_user_id,
        updated_at = current_timestamp
    where singleton
    returning jsonb_build_object(
        'auto_publish_enabled', auto_publish_enabled,
        'updated_by', updated_by,
        'updated_at', updated_at
    );
$$ language sql;

-- Updates source trust and records Moderator confirmation for trusted sources.
create or replace function set_talent_source_status(
    p_source_id uuid,
    p_trust_status text,
    p_actor_user_id uuid,
    p_auto_publish_enabled boolean
)
returns table(
    source_id uuid,
    trust_status text,
    auto_publish_enabled boolean,
    confirmed_by uuid,
    confirmed_at timestamptz
) as $$
begin
    if p_trust_status not in ('new', 'trusted', 'watchlist', 'blocked') then
        raise exception 'invalid talent source trust status' using errcode = '22023';
    end if;
    if p_trust_status = 'trusted' and p_actor_user_id is null then
        raise exception 'trusted talent source requires a Moderator confirmer'
            using errcode = '22023';
    end if;

    return query
    update talent_sources as updated_source
    set
        trust_status = p_trust_status,
        auto_publish_enabled = case
            when p_trust_status in ('watchlist', 'blocked', 'new') then false
            else coalesce(p_auto_publish_enabled, false)
        end,
        confirmed_by = case
            when p_trust_status = 'trusted' then p_actor_user_id
            else confirmed_by
        end,
        confirmed_at = case
            when p_trust_status = 'trusted' then current_timestamp
            else confirmed_at
        end,
        updated_at = current_timestamp
    where updated_source.source_id = p_source_id
    returning
        updated_source.source_id,
        updated_source.trust_status,
        updated_source.auto_publish_enabled,
        updated_source.confirmed_by,
        updated_source.confirmed_at;
end;
$$ language plpgsql;

create or replace function set_talent_source_subscription_enabled(
    p_subscription_id uuid,
    p_enabled boolean,
    p_actor_user_id uuid
)
returns boolean as $$
declare
    v_updated integer;
begin
    if not exists (select 1 from "user" where user_id = p_actor_user_id and moderator) then
        raise exception 'talent source subscription update requires a moderator' using errcode = '42501';
    end if;
    update talent_source_subscriptions set
        enabled = p_enabled,
        claimed_at = case when p_enabled then claimed_at else null end,
        next_run_at = case when p_enabled then least(next_run_at, current_timestamp) else next_run_at end,
        updated_at = current_timestamp
    where subscription_id = p_subscription_id;
    get diagnostics v_updated = row_count;
    return v_updated > 0;
end;
$$ language plpgsql;

-- Applies non-publication review state transitions with optimistic concurrency.
create or replace function transition_talent_import_item(
    p_item_id uuid,
    p_lock_version integer,
    p_next_status text,
    p_actor_user_id uuid,
    p_reason text default null
)
returns boolean as $$
declare
    v_status text;
    v_batch_id uuid;
begin
    if p_reason is not null
       and p_reason not in (
           'duplicate',
           'rejected',
           'reprocess',
           'withdrawn',
           'moderator_correction',
           'policy_violation',
           'expired',
           'other'
       ) then
        raise exception 'invalid talent audit reason' using errcode = '22023';
    end if;

    select status, batch_id
    into v_status, v_batch_id
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if not exists (
        select 1
        from talent_import_items
        where item_id = p_item_id
          and lock_version = p_lock_version
    ) then
        return false;
    end if;
    if not (
        (v_status = 'needs_review' and p_next_status in ('rejected', 'queued'))
        or (v_status in ('rejected', 'failed') and p_next_status = 'queued')
    ) then
        raise exception 'invalid talent import item transition' using errcode = '22023';
    end if;

    update talent_import_items
    set
        status = p_next_status,
        lock_version = lock_version + 1,
        processed_at = case when p_next_status = 'queued' then null else processed_at end,
        reviewed_at = case when p_next_status = 'queued' then null else current_timestamp end,
        updated_at = current_timestamp
    where item_id = p_item_id;

    if p_next_status = 'queued' then
        update talent_import_batches
        set
            processed_count = processed_count - 1,
            failed_count = failed_count - case when v_status = 'failed' then 1 else 0 end,
            status = 'queued',
            completed_at = null
        where batch_id = v_batch_id
          and processed_count > 0
          and (v_status <> 'failed' or failed_count > 0);

        if not found then
            raise exception 'talent import batch counters are invalid';
        end if;
    end if;

    insert into talent_review_events (
        item_id,
        actor_type,
        actor_user_id,
        action,
        reason,
        before_snapshot,
        after_snapshot
    )
    values (
        p_item_id,
        'moderator',
        p_actor_user_id,
        p_next_status,
        p_reason,
        jsonb_build_object('status', v_status, 'lock_version', p_lock_version),
        jsonb_build_object('status', p_next_status, 'lock_version', p_lock_version + 1)
    );

    return true;
end;
$$ language plpgsql;

-- Saves editable safe review fields with optimistic concurrency.
create or replace function update_talent_review_item(
    p_item_id uuid,
    p_lock_version integer,
    p_updates jsonb,
    p_actor_user_id uuid
)
returns table(item_id uuid, lock_version integer) as $$
declare
    v_item_id uuid;
    v_lock_version integer;
begin
    if p_updates is null or jsonb_typeof(p_updates) <> 'object' then
        raise exception 'talent review update must be a JSON object' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(p_updates) as fields(field)
        where field <> all(
            array[
                'kind',
                'extraction',
                'extraction_confidence',
                'contact_masked',
                'validation_errors',
                'risk_score',
                'risk_level',
                'risk_reasons',
                'duplicate_candidates',
                'normalized_fingerprint',
                'manual_policy_reasons'
            ]
        )
    ) then
        raise exception 'talent review update contains an unknown field' using errcode = '22023';
    end if;
    if jsonb_typeof(coalesce(p_updates->'extraction', '{}'::jsonb)) <> 'object' then
        raise exception 'talent review extraction must be a JSON object' using errcode = '22023';
    end if;
    if coalesce(p_updates->'extraction', '{}'::jsonb) ? 'contacts' then
        raise exception 'complete contacts are forbidden in safe talent extraction' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(coalesce(p_updates->'extraction', '{}'::jsonb)) as fields(field)
        where field <> all(
            array[
                'kind',
                'confidence',
                'title',
                'scene',
                'region',
                'city',
                'district',
                'salaryMin',
                'salaryMax',
                'roles',
                'skills',
                'experienceYears',
                'hoursPerDay',
                'restDaysPerMonth',
                'lodging',
                'housingAllowance',
                'availability',
                'urgency',
                'publishedAt',
                'missingFields',
                'riskCandidates'
            ]
        )
    ) then
        raise exception 'talent review extraction contains an unknown field' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(coalesce(p_updates->'extraction', '{}'::jsonb)) as fields(field, value)
        where case
            when field = 'kind' then
                jsonb_typeof(value) <> 'string'
                or value #>> '{}' not in ('hiring', 'seeking', 'irrelevant', 'uncertain')
            when field = 'confidence' then
                jsonb_typeof(value) <> 'number'
                or case
                    when jsonb_typeof(value) = 'number'
                        then (value #>> '{}')::numeric not between 0 and 1
                    else false
                end
            when field in (
                'title', 'scene', 'region', 'city', 'district',
                'lodging', 'availability', 'urgency', 'publishedAt'
            ) then jsonb_typeof(value) not in ('string', 'null')
            when field in (
                'salaryMin', 'salaryMax', 'experienceYears', 'hoursPerDay',
                'restDaysPerMonth', 'housingAllowance'
            ) then jsonb_typeof(value) not in ('number', 'null')
            when field in ('roles', 'skills', 'missingFields', 'riskCandidates')
                then jsonb_typeof(value) <> 'array'
            else true
        end
    ) then
        raise exception 'talent review extraction contains an invalid field type'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(coalesce(p_updates->'extraction', '{}'::jsonb)) as arrays(field, value)
        cross join lateral jsonb_array_elements(
            case when jsonb_typeof(value) = 'array' then value else '[]'::jsonb end
        ) as entries(entry)
        where field in ('roles', 'skills', 'missingFields')
          and (
              jsonb_typeof(entry) <> 'string'
              or coalesce(entry #>> '{}', '') = ''
          )
    ) then
        raise exception 'talent review extraction contains an invalid array entry'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_updates->'extraction'->'riskCandidates') = 'array'
                    then p_updates->'extraction'->'riskCandidates'
                else '[]'::jsonb
            end
        ) as risks(risk)
        where jsonb_typeof(risk) <> 'object'
           or not (risk ?& array['code', 'explanation', 'evidence', 'hard_block'])
           or exists (
               select 1
               from jsonb_object_keys(
                   case when jsonb_typeof(risk) = 'object' then risk else '{}'::jsonb end
               ) as keys(key)
               where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
           )
           or jsonb_typeof(risk->'code') <> 'string'
           or coalesce(risk->>'code', '') = ''
           or jsonb_typeof(risk->'explanation') <> 'string'
           or coalesce(risk->>'explanation', '') = ''
           or jsonb_typeof(risk->'evidence') not in ('string', 'null')
           or jsonb_typeof(risk->'hard_block') <> 'boolean'
    ) then
        raise exception 'talent review extraction contains an invalid risk candidate'
            using errcode = '22023';
    end if;
    if p_updates ? 'contact_masked'
       and jsonb_typeof(p_updates->'contact_masked') <> 'array' then
        raise exception 'talent review masked contacts must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'contact_masked'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'contact_masked') = 'array'
                       then p_updates->'contact_masked'
                   else '[]'::jsonb
               end
           ) as contacts(contact)
           where jsonb_typeof(contact) <> 'object'
              or not (contact ?& array['contact_type', 'masked_value'])
              or exists (
                  select 1
                  from jsonb_object_keys(
                      case when jsonb_typeof(contact) = 'object' then contact else '{}'::jsonb end
                  ) as keys(key)
                  where key <> all(array['contact_type', 'masked_value'])
              )
              or coalesce(contact->>'contact_type', '') not in ('phone', 'wechat', 'qq', 'other')
              or char_length(coalesce(contact->>'masked_value', '')) > 64
              or coalesce(contact->>'masked_value', '')
                  !~ '^[^*[:space:][:cntrl:]]{0,4}\*{3,}[^*[:space:][:cntrl:]]{0,4}$'
       ) then
        raise exception 'invalid masked talent review contact' using errcode = '22023';
    end if;
    if p_updates ? 'validation_errors'
       and jsonb_typeof(p_updates->'validation_errors') <> 'array' then
        raise exception 'talent review validation errors must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'risk_reasons'
       and jsonb_typeof(p_updates->'risk_reasons') <> 'array' then
        raise exception 'talent review risk reasons must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'risk_reasons'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'risk_reasons') = 'array'
                       then p_updates->'risk_reasons'
                   else '[]'::jsonb
               end
           ) as reasons(reason)
           where jsonb_typeof(reason) <> 'object'
              or not (reason ?& array['code', 'explanation', 'evidence', 'hard_block'])
              or exists (
                  select 1
                  from jsonb_object_keys(
                      case when jsonb_typeof(reason) = 'object' then reason else '{}'::jsonb end
                  ) as keys(key)
                  where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
              )
              or jsonb_typeof(reason->'code') <> 'string'
              or coalesce(reason->>'code', '') = ''
              or jsonb_typeof(reason->'explanation') <> 'string'
              or coalesce(reason->>'explanation', '') = ''
              or jsonb_typeof(reason->'evidence') not in ('string', 'null')
              or jsonb_typeof(reason->'hard_block') <> 'boolean'
       ) then
        raise exception 'talent review risk reasons contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_updates ? 'duplicate_candidates'
       and jsonb_typeof(p_updates->'duplicate_candidates') <> 'array' then
        raise exception 'talent review duplicate candidates must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'duplicate_candidates'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'duplicate_candidates') = 'array'
                       then p_updates->'duplicate_candidates'
                   else '[]'::jsonb
               end
           ) as candidates(candidate)
           where jsonb_typeof(candidate) <> 'object'
              or not (
                  candidate ?& array[
                      'candidate_item_id',
                      'candidate_record_id',
                      'score',
                      'reasons',
                      'high_similarity'
                  ]
              )
              or exists (
                  select 1
                  from jsonb_object_keys(
                      case
                          when jsonb_typeof(candidate) = 'object' then candidate
                          else '{}'::jsonb
                      end
                  ) as keys(key)
                  where key <> all(
                      array[
                          'candidate_item_id',
                          'candidate_record_id',
                          'score',
                          'reasons',
                          'high_similarity'
                      ]
                  )
              )
              or jsonb_typeof(candidate->'candidate_item_id') not in ('string', 'null')
              or jsonb_typeof(candidate->'candidate_record_id') not in ('string', 'null')
              or (
                  jsonb_typeof(candidate->'candidate_item_id') = 'null'
                  and jsonb_typeof(candidate->'candidate_record_id') = 'null'
              )
              or (
                  jsonb_typeof(candidate->'candidate_item_id') = 'string'
                  and (candidate->>'candidate_item_id')
                      !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
              )
              or (
                  jsonb_typeof(candidate->'candidate_record_id') = 'string'
                  and (candidate->>'candidate_record_id')
                      !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
              )
              or jsonb_typeof(candidate->'score') <> 'number'
              or case
                  when jsonb_typeof(candidate->'score') = 'number'
                      then (candidate->>'score')::numeric not between 0 and 1
                  else false
              end
              or jsonb_typeof(candidate->'reasons') <> 'array'
              or exists (
                  select 1
                  from jsonb_array_elements(
                      case
                          when jsonb_typeof(candidate->'reasons') = 'array'
                              then candidate->'reasons'
                          else '[]'::jsonb
                      end
                  ) as duplicate_reasons(reason)
                  where jsonb_typeof(reason) <> 'string'
                     or coalesce(reason #>> '{}', '') = ''
              )
              or jsonb_typeof(candidate->'high_similarity') <> 'boolean'
       ) then
        raise exception 'talent review duplicate candidates contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_updates ? 'manual_policy_reasons'
       and jsonb_typeof(p_updates->'manual_policy_reasons') <> 'array' then
        raise exception 'talent review manual policy reasons must be a JSON array'
            using errcode = '22023';
    end if;
    if p_updates ? 'manual_policy_reasons'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'manual_policy_reasons') = 'array'
                       then p_updates->'manual_policy_reasons'
                   else '[]'::jsonb
               end
           ) as reasons(reason)
           where jsonb_typeof(reason) <> 'string'
              or (reason #>> '{}') not in (
                  'global_auto_publish_paused',
                  'source_not_trusted',
                  'source_auto_publish_paused',
                  'source_not_confirmed',
                  'risk_version_mismatch',
                  'risk_not_low',
                  'risk_hard_block',
                  'unsupported_kind',
                  'confidence_below_threshold',
                  'missing_required_fields',
                  'duplicate_high'
              )
       ) then
        raise exception 'talent review manual policy reasons contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_updates ? 'normalized_fingerprint'
       and (
           jsonb_typeof(p_updates->'normalized_fingerprint') <> 'string'
           or (p_updates->>'normalized_fingerprint') !~ '^[0-9a-f]{64}$'
       ) then
        raise exception 'talent review normalized fingerprint is invalid'
            using errcode = '22023';
    end if;

    update talent_import_items as reviewed
    set
        kind = coalesce(nullif(p_updates->>'kind', ''), reviewed.kind),
        extraction = (
            coalesce(reviewed.extraction, '{}'::jsonb)
            || coalesce(p_updates->'extraction', '{}'::jsonb)
        ) - 'contacts',
        extraction_confidence = coalesce(
            nullif(p_updates->>'extraction_confidence', '')::numeric,
            reviewed.extraction_confidence
        ),
        contact_masked = coalesce(p_updates->'contact_masked', reviewed.contact_masked),
        validation_errors = coalesce(p_updates->'validation_errors', reviewed.validation_errors),
        risk_score = coalesce(nullif(p_updates->>'risk_score', '')::numeric, reviewed.risk_score),
        risk_level = coalesce(nullif(p_updates->>'risk_level', ''), reviewed.risk_level),
        risk_reasons = coalesce(p_updates->'risk_reasons', reviewed.risk_reasons),
        duplicate_candidates = coalesce(
            p_updates->'duplicate_candidates',
            reviewed.duplicate_candidates
        ),
        normalized_fingerprint = coalesce(
            nullif(p_updates->>'normalized_fingerprint', ''),
            reviewed.normalized_fingerprint
        ),
        lock_version = reviewed.lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where reviewed.item_id = p_item_id
      and reviewed.lock_version = p_lock_version
      and reviewed.status = 'needs_review'
    returning reviewed.item_id, reviewed.lock_version
    into v_item_id, v_lock_version;

    if v_item_id is null then
        return;
    end if;

    insert into talent_review_events (
        item_id,
        actor_type,
        actor_user_id,
        action,
        before_snapshot,
        after_snapshot
    )
    values (
        v_item_id,
        'moderator',
        p_actor_user_id,
        'review_updated',
        jsonb_build_object('lock_version', p_lock_version),
        jsonb_build_object(
            'lock_version', v_lock_version,
            'manual_policy_reasons',
            coalesce(p_updates->'manual_policy_reasons', '[]'::jsonb)
        )
    );

    item_id := v_item_id;
    lock_version := v_lock_version;
    return next;
end;
$$ language plpgsql;

-- Creates a source or returns the existing case-insensitive source identity.
create or replace function upsert_talent_source(
    p_source_type text,
    p_name text,
    p_created_by uuid default null
)
returns uuid as $$
declare
    v_source_id uuid;
begin
    if p_source_type not in ('wechat_group', 'qq_group', 'web', 'other') then
        raise exception 'invalid talent source type' using errcode = '22023';
    end if;
    if nullif(btrim(p_name), '') is null then
        raise exception 'talent source name cannot be empty' using errcode = '22023';
    end if;

    insert into talent_sources (source_type, name, created_by)
    values (p_source_type, btrim(p_name), p_created_by)
    on conflict (source_type, lower(name))
    do update set updated_at = current_timestamp
    returning source_id into v_source_id;

    return v_source_id;
end;
$$ language plpgsql;

create or replace function upsert_talent_source_subscription(p_subscription jsonb)
returns uuid as $$
declare
    v_id uuid;
    v_url text := trim(p_subscription->>'public_url');
    v_keywords text[];
begin
    if p_subscription is null or jsonb_typeof(p_subscription) <> 'object' then
        raise exception 'source subscription must be an object' using errcode = '22023';
    end if;
    if not exists (
        select 1 from "user"
        where user_id = nullif(p_subscription->>'actor_user_id', '')::uuid
          and moderator
    ) then
        raise exception 'source subscription update requires a moderator' using errcode = '42501';
    end if;
    if v_url !~ '^https://[^/?#[:space:]]+(?:[/?][^#[:space:]]*)?$' or v_url ~ '@' then
        raise exception 'source subscription requires a public HTTPS URL' using errcode = '22023';
    end if;
    select array_agg(keyword order by keyword) into v_keywords
    from (
        select distinct lower(trim(value)) keyword
        from jsonb_array_elements_text(p_subscription->'keywords') values_list(value)
        where trim(value) <> '' and length(trim(value)) <= 50
    ) normalized;
    if coalesce(cardinality(v_keywords), 0) not between 1 and 20 then
        raise exception 'source subscription requires 1 to 20 keywords' using errcode = '22023';
    end if;

    if nullif(p_subscription->>'subscription_id', '') is not null then
        update talent_source_subscriptions set
            public_url = v_url,
            keywords = v_keywords,
            interval_minutes = (p_subscription->>'interval_minutes')::integer,
            enabled = coalesce((p_subscription->>'enabled')::boolean, true),
            next_run_at = case when coalesce((p_subscription->>'enabled')::boolean, true) then least(next_run_at, current_timestamp) else next_run_at end,
            updated_at = current_timestamp
        where subscription_id = (p_subscription->>'subscription_id')::uuid
          and source_id = (p_subscription->>'source_id')::uuid
        returning subscription_id into v_id;
        if v_id is null then
            raise exception 'source subscription not found' using errcode = 'P0002';
        end if;
        return v_id;
    end if;

    insert into talent_source_subscriptions (
        subscription_id, source_id, public_url, keywords, interval_minutes, enabled, created_by
    ) values (
        coalesce(nullif(p_subscription->>'subscription_id', '')::uuid, gen_random_uuid()),
        (p_subscription->>'source_id')::uuid,
        v_url,
        v_keywords,
        (p_subscription->>'interval_minutes')::integer,
        coalesce((p_subscription->>'enabled')::boolean, true),
        nullif(p_subscription->>'actor_user_id', '')::uuid
    )
    on conflict (source_id, public_url) do update set
        keywords = excluded.keywords,
        interval_minutes = excluded.interval_minutes,
        enabled = excluded.enabled,
        next_run_at = case when excluded.enabled then least(talent_source_subscriptions.next_run_at, current_timestamp) else talent_source_subscriptions.next_run_at end,
        updated_at = current_timestamp
    returning subscription_id into v_id;
    return v_id;
end;
$$ language plpgsql;

-- Withdraws a formal record and records a contact-free review event when linked.
create or replace function withdraw_talent_record(
    p_record_id uuid,
    p_actor_user_id uuid,
    p_reason text
)
returns boolean as $$
declare
    v_source_id uuid;
    v_item_id uuid;
begin
    if p_reason is not null
       and p_reason not in (
           'duplicate',
           'rejected',
           'reprocess',
           'withdrawn',
           'moderator_correction',
           'policy_violation',
           'expired',
           'other'
       ) then
        raise exception 'invalid talent audit reason' using errcode = '22023';
    end if;

    update talent_records
    set
        status = 'withdrawn',
        withdrawn_at = coalesce(withdrawn_at, current_timestamp),
        updated_at = current_timestamp
    where record_id = p_record_id
      and status <> 'withdrawn'
    returning source_id into v_source_id;

    if not found then
        return exists (
            select 1 from talent_records
            where record_id = p_record_id and status = 'withdrawn'
        );
    end if;

    select item_id
    into v_item_id
    from talent_import_items
    where published_record_id = p_record_id;

    if v_item_id is not null then
        insert into talent_review_events (
            item_id,
            record_id,
            actor_type,
            actor_user_id,
            action,
            reason,
            before_snapshot,
            after_snapshot
        )
        values (
            v_item_id,
            p_record_id,
            'moderator',
            p_actor_user_id,
            'withdrawn',
            p_reason,
            jsonb_build_object('record_status', 'published'),
            jsonb_build_object('record_status', 'withdrawn')
        );
    end if;

    if v_source_id is not null then
        update talent_sources
        set withdrawn_count = withdrawn_count + 1, updated_at = current_timestamp
        where source_id = v_source_id;
    end if;

    return true;
end;
$$ language plpgsql;

-- Archives published jobs that expired more than 30 days ago.
create or replace function archive_expired_jobs()
returns void as $$
    update job
    set
        status = 'archived',
        archived_at = current_timestamp,
        updated_at = current_timestamp
    where status = 'published'
    and published_at + '30 days'::interval < current_timestamp;
$$ language sql;

-- Preserves audit structure while replacing every free-text leaf with a digest.
create or replace function talent_audit_safe_jsonb(p_value jsonb)
returns jsonb as $$
declare
    v_kind text;
    v_result jsonb;
    v_text text;
begin
    if p_value is null then
        return null;
    end if;

    v_kind := jsonb_typeof(p_value);
    case v_kind
        when 'object' then
            select coalesce(
                jsonb_object_agg(entry.key, talent_audit_safe_jsonb(entry.value)),
                '{}'::jsonb
            )
            into v_result
            from jsonb_each(p_value) as entry;
            return v_result;
        when 'array' then
            select coalesce(
                jsonb_agg(talent_audit_safe_jsonb(entry.value) order by entry.ordinality),
                '[]'::jsonb
            )
            into v_result
            from jsonb_array_elements(p_value) with ordinality as entry(value, ordinality);
            return v_result;
        when 'string' then
            v_text := p_value #>> '{}';
            return jsonb_build_object(
                'redacted', true,
                'sha256', encode(digest(convert_to(v_text, 'UTF8'), 'sha256'), 'hex'),
                'character_count', char_length(v_text)
            );
        when 'number', 'boolean', 'null' then
            return p_value;
        else
            raise exception 'unsupported talent audit JSON value' using errcode = '22023';
    end case;
end;
$$ language plpgsql immutable parallel safe;

create or replace function talent_review_state_snapshot(p_item talent_import_items)
returns jsonb as $$
    select jsonb_build_object(
        'kind', p_item.kind,
        'extraction', talent_audit_safe_jsonb(coalesce(p_item.extraction, '{}'::jsonb)),
        'extraction_confidence', p_item.extraction_confidence,
        'contact_masked', talent_audit_safe_jsonb(coalesce(p_item.contact_masked, '[]'::jsonb)),
        'validation_errors', talent_audit_safe_jsonb(coalesce(p_item.validation_errors, '[]'::jsonb)),
        'risk_score', p_item.risk_score,
        'risk_level', p_item.risk_level,
        'risk_reasons', talent_audit_safe_jsonb(coalesce(p_item.risk_reasons, '[]'::jsonb)),
        'duplicate_candidates', talent_audit_safe_jsonb(
            coalesce(p_item.duplicate_candidates, '[]'::jsonb)
        ),
        'normalized_fingerprint_present', nullif(p_item.normalized_fingerprint, '') is not null,
        'lock_version', p_item.lock_version,
        'status', p_item.status
    );
$$ language sql immutable parallel safe;
